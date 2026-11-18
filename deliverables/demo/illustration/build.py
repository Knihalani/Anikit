#!/usr/bin/env python3
"""Burrow launch film ("The bunny who lost the moon"): builds spec.json.

One bedtime, one story made up out loud. A parent starts it ("Once upon a
tonight,"), the child finishes the sentence in crayon ("a bunny!"), and the
camera falls into the quilt, whose patchwork hills become the story's country.
The device is the moon: missing from the bedroom window, a crayon outline in
the story sky, a reflection in a pond, fished out and hung back up, and finally
back in the real window over a sleeping child. Two voices in the type: the
parent's soft serif and the child's crayon hand.

Everything is components: `illo` for the room and the bunny (lit flat shapes,
one light, rig with lag and springs), `landscape`/`water`/`plants` for the
story country. No hand-built shape dumps.
"""
import json, os

HERE = os.path.dirname(os.path.abspath(__file__))

# ------------------------------------------------------------------ palette
NIGHT = "#141a46"
WALL = "#232a63"
WALL_HI = "#2f3a7e"
CREAM = "#f6e6c4"
CRAYON = "#ff8f45"
GOLD = "#ffcf6a"
INK = "#120d24"
SKIN = "#eab08e"
SKIN2 = "#c98a6c"
HAIR_K = "#3a1f2e"
HAIR_P = "#5b2a2a"
QUILT = ["#6b3a6e", "#2f6f73", "#d39a3c", "#c76a5a", "#3c4a8a", "#8a4f3a"]
BUNNY = "#efe3d0"

PARENT = {"font": "Fraunces", "axes": {"SOFT": 100, "WONK": 1, "opsz": 144, "wght": 380}, "color": CREAM}
CHILD = {"font": "Shantell Sans", "axes": {"INFM": 100, "BNCE": 100, "SPAC": 0, "wght": 720}, "color": CRAYON}


def sh(shape, **k):
    d = {"shape": shape}
    d.update(k)
    return d


def illo(shapes, groups=(), light=None, **extra):
    props = {
        "view": [1920, 1080],
        "light": light or {"angle": 215, "rim": 40, "color": "#ffd59a", "shadow": "#2b1f5c", "strength": 1.0},
        "hand": {"weight": [0.7, 3.6], "wobble": 1.2, "rough": 0.6, "boil": 8, "boilAmount": 0.6, "gaps": 0.2},
        "corners": "soft", "misregister": [1.5, 1], "grain": 0.06, "onTwos": True,
        "groups": list(groups), "shapes": shapes,
    }
    props.update(extra)
    return {"component": "illo", "props": props}


def lit(layer):
    """Split an illo layer: shapes marked blend=screen go to a second illo layer composited with
    a layer-level screen blend (light adds to everything below it, not only to the illo's own shapes)."""
    p = layer["props"]
    glow = [dict(x, blend="normal") for x in p["shapes"] if x.get("blend") == "screen"]
    if not glow:
        return [layer]
    for x in glow:
        x.pop("clipTo", None)
    p["shapes"] = [x for x in p["shapes"] if x.get("blend") != "screen"]
    gp = dict(p, shapes=glow, grain=0, misregister=[0, 0], hand=dict(p["hand"], boil=0))
    return [layer, {"component": "illo", "blend": "screen", "props": gp}]


# ------------------------------------------------------------------ the bunny
def hops(x0, x1, t0, t1, n, h, s=1.0, hold=None):
    """Root keyframes for n hops from x0 to x1: crouch (anticipation), stretch in the air, squash on landing, settle."""
    keys = [{"t": 0, "x": x0, "y": 0, "squash": 1.0}]
    if t0 > 0:
        keys.append({"t": t0, "x": x0, "y": 0, "squash": 1.0})
    dur = (t1 - t0) / n
    for i in range(n):
        a = t0 + i * dur
        xa = x0 + (x1 - x0) * i / n
        xb = x0 + (x1 - x0) * (i + 1) / n
        keys += [
            {"t": round(a + 0.18 * dur, 3), "x": xa, "y": 0, "squash": 0.88, "ease": "out"},
            {"t": round(a + 0.5 * dur, 3), "x": (xa + xb) / 2, "y": -h * s, "squash": 1.08, "ease": "out"},
            {"t": round(a + 0.78 * dur, 3), "x": xb, "y": 0, "squash": 1.04, "ease": "in"},
            {"t": round(a + 0.88 * dur, 3), "x": xb, "y": 0, "squash": 0.88, "ease": "out"},
            {"t": round(a + dur, 3), "x": xb, "y": 0, "squash": 1.0, "ease": "out"},
        ]
    if hold:
        keys.append({"t": hold, "x": x1, "y": 0, "squash": 1.0})
    return keys


def bunny(px, x, y, s=1.0, walk=None, lantern=True, net=False, sleep=False, look_down=False, delay=0.0, keys=None):
    """Shapes + groups for the bunny, feet at (x, y), facing right. `walk` is a list of
    group keyframes for the root (travel)."""
    def X(v):
        return round(x + s * v, 1)

    def Y(v):
        return round(y + s * v, 1)

    g = []
    root = {"name": px + "root", "pivot": [X(0), Y(0)]}
    if walk:
        root["animate"] = {"keys": walk, "ease": "inOut"}
    if keys:
        root["animate"] = {"keys": keys, "ease": "inOut"}
    g.append(root)
    body = {"name": px + "body", "parent": px + "root", "pivot": [X(0), Y(0)],
            "cycle": {"style": "breathe", "amount": 1.4}}
    g.append(body)
    head = {"name": px + "head", "parent": px + "body", "pivot": [X(20), Y(-120)], "lag": 0.04,
            "cycle": {"style": "nod", "speed": 0.4, "amount": 1.4}}
    if look_down:
        head["rotate"] = 14
    g.append(head)
    g.append({"name": px + "ears", "parent": px + "head", "pivot": [X(28), Y(-196)],
              "spring": {"stiffness": 120, "damping": 8, "drag": 0.08}, "rotate": -8 if not sleep else -60})
    g.append({"name": px + "eye", "parent": px + "head", "pivot": [X(54), Y(-168)],
              "blink": {"every": 2.6, "seed": len(px)}})
    sh_ = []
    sh_.append(sh("ellipse", cx=X(5), cy=Y(4), rx=80 * s, ry=12 * s, fill=INK, opacity=0.35, outline=False, group=px + "root"))
    body_g, head_g = px + "body", px + "head"
    sh_.append(sh("circle", cx=X(-62), cy=Y(-52), r=20 * s, fill=BUNNY, shade={"amount": 0.5}, group=body_g))
    sh_.append(sh("blob", name=px + "torso", cx=X(0), cy=Y(-70), rx=64 * s, ry=72 * s, lumpy=0.08, n=8, seed=3, fill=BUNNY,
                  shade={"amount": 0.75, "offset": 0.16, "rim": 0.04}, texture={"style": "contour", "amount": 0.35},
                  group=body_g))
    sh_.append(sh("ellipse", cx=X(30), cy=Y(-4), rx=34 * s, ry=12 * s, fill=BUNNY, shade={"amount": 0.5}, group=body_g))
    # ears (behind head first)
    sh_.append(sh("ellipse", cx=X(14), cy=Y(-246), rx=15 * s, ry=58 * s, rotate=-12, fill=BUNNY, shade={"amount": 0.7},
                  group=px + "ears"))
    sh_.append(sh("ellipse", name=px + "earR", cx=X(44), cy=Y(-252), rx=16 * s, ry=60 * s, rotate=10, fill=BUNNY,
                  shade={"amount": 0.6}, group=px + "ears"))
    sh_.append(sh("ellipse", cx=X(46), cy=Y(-248), rx=7 * s, ry=42 * s, rotate=10, fill="#e79a9a", opacity=0.85,
                  clipTo=px + "earR", outline=False, group=px + "ears"))
    sh_.append(sh("circle", name=px + "skull", cx=X(34), cy=Y(-160), r=50 * s, fill=BUNNY,
                  shade={"amount": 0.7, "offset": 0.14, "rim": 0.05}, group=head_g))
    sh_.append(sh("circle", cx=X(66), cy=Y(-140), r=12 * s, fill="#ef9a8a", opacity=0.45, clipTo=px + "skull", outline=False,
                  group=head_g))
    if sleep:
        sh_.append(sh("path", d=f"M{X(46)} {Y(-166)} q {8*s} {8*s} {16*s} 0", stroke=INK, weight=[2, 4], group=head_g))
    else:
        sh_.append(sh("ellipse", cx=X(56), cy=Y(-168), rx=6.5 * s, ry=9 * s, fill=INK, outline=False, group=px + "eye"))
        sh_.append(sh("circle", cx=X(58), cy=Y(-171), r=2.2 * s, fill="#ffffff", outline=False, group=px + "eye"))
    sh_.append(sh("circle", cx=X(83), cy=Y(-152), r=6 * s, fill="#d9707a", outline=False, group=head_g))
    if lantern:
        g.append({"name": px + "lamp", "parent": px + "body", "pivot": [X(70), Y(-92)], "lag": 0.12,
                  "spring": {"stiffness": 55, "damping": 4, "drag": 0.5}})
        sh_.append(sh("path", points=[[X(30), Y(-96)], [X(70), Y(-92)]], stroke=BUNNY, weight=[6 * s, 9 * s], group=body_g))
        sh_.append(sh("path", points=[[X(70), Y(-92)], [X(70), Y(-60)]], stroke=INK, weight=[2, 3], group=px + "lamp"))
        sh_.append(sh("circle", cx=X(70), cy=Y(-34), r=70 * s, fill={"stops": [[0, GOLD], [0.62, "#000000"], [1, "#000000"]], "gradient": "radial"},
                      opacity=0.55, blend="screen", outline=False, group=px + "lamp"))
        sh_.append(sh("ellipse", cx=X(60), cy=Y(4), rx=150 * s, ry=30 * s, fill={"stops": [[0, "#c89a4a"], [0.7, "#000000"], [1, "#000000"]], "gradient": "radial"},
                      blend="screen", outline=False, group=px + "root"))
        sh_.append(sh("circle", cx=X(30), cy=Y(-80), r=110 * s, fill={"stops": [[0, "#6a5030"], [0.7, "#000000"], [1, "#000000"]], "gradient": "radial"},
                      blend="screen", outline=False, group=px + "body"))
        sh_.append(sh("rect", cx=X(70), cy=Y(-36), w=26 * s, h=34 * s, radius=6 * s, fill=GOLD,
                      shade={"amount": 0.3, "offset": 0.1}, lineColor="#7a3b1e", group=px + "lamp"))
        sh_.append(sh("rect", cx=X(70), cy=Y(-56), w=30 * s, h=7 * s, radius=2, fill="#7a3b1e", group=px + "lamp"))
    if net:
        g.append({"name": px + "net", "parent": px + "body", "pivot": [X(40), Y(-96)], "lag": 0.14,
                  "animate": {"keys": net, "ease": "inOut"}})
        sh_.append(sh("path", points=[[X(30), Y(-96)], [X(200), Y(-40)]], stroke="#8a5a3a", weight=[5 * s, 7 * s], group=px + "net"))
        sh_.append(sh("ellipse", cx=X(240), cy=Y(-20), rx=46 * s, ry=26 * s, rotate=-20, fill="none", stroke="#8a5a3a",
                      weight=[3, 5], group=px + "net"))
    # paw in front
    sh_.append(sh("ellipse", cx=X(30), cy=Y(-92), rx=14 * s, ry=11 * s, fill=BUNNY, shade={"amount": 0.4}, group=body_g))
    return g, sh_


# ------------------------------------------------------------------ the bedroom
def bedroom(night_end=False):
    """The room. In the story beat a lamp is on and a parent sits on the bed; at
    the end the lamp is off, the child sleeps and the moon is back in the window."""
    S, G = [], []
    S.append(sh("rect", x=-40, y=-40, w=2000, h=1160, fill={"stops": [[0, WALL_HI], [1, WALL]], "angle": 80, "steps": 4, "dither": 0.15},
                wobble=0, rough=0, grain=0.07, corners="sharp", texture={"style": "riso", "amount": 0.3}))
    # window
    S.append(sh("rect", name="win", x=1270, y=110, w=430, h=420, radius=6, fill={"stops": [[0, "#0a0f33"], [1, "#24306e"]], "angle": 90, "steps": 4},
                shadow={"dist": 0, "dx": -18, "dy": 16, "color": "#10153a", "opacity": 0.6}))
    for (sx, sy, r) in [(1330, 170, 6), (1420, 240, 4), (1640, 180, 7), (1560, 300, 4), (1360, 380, 5), (1660, 420, 4), (1480, 150, 3)]:
        S.append(sh("star", cx=sx, cy=sy, r=r * 1.6, n=4, inner=0.35, fill=CREAM, outline=False, clipTo="win",
                    sway={"scale": 0.35, "period": 2 + (sx % 7) / 3, "phase": (sy % 10) / 10}))
    if night_end:
        S.append(sh("circle", cx=1520, cy=290, r=150, fill={"stops": [[0, "#ffe7a6"], [0.62, "#000000"], [1, "#000000"]], "gradient": "radial"},
                    opacity=0.7, blend="screen", outline=False, clipTo="win"))
        S.append(sh("circle", name="mcut", cx=1555, cy=265, r=70, visible=False))
        S.append(sh("circle", cx=1520, cy=290, r=78, fill="#ffe3a0", cut=["mcut"], shade={"amount": 0.3, "offset": 0.08},
                    texture={"style": "halftone", "amount": 0.35, "scale": 7}, clipTo="win"))
    # window frame + cross
    S.append(sh("rect", x=1262, y=102, w=446, h=436, radius=8, fill="none", stroke="#d9c29a", weight=[10, 16]))
    S.append(sh("path", points=[[1485, 110], [1485, 530]], stroke="#d9c29a", weight=[9, 12]))
    S.append(sh("path", points=[[1270, 320], [1700, 320]], stroke="#d9c29a", weight=[9, 12]))
    # curtain
    S.append(sh("path", d="M1200 70 C 1260 240, 1210 420, 1250 620 L 1170 640 C 1150 420, 1190 240, 1140 70 Z", fill="#8a3c52",
                shade={"amount": 0.7, "offset": 0.2}, texture={"style": "contour", "amount": 0.4}, sway={"x": 4, "period": 5}))
    S.append(sh("path", d="M1100 60 L 1780 60", stroke="#4a2a2a", weight=[8, 10]))
    # crayon drawings pinned to the wall
    S.append(sh("rect", name="pic", x=860, y=150, w=200, h=150, rotate=-4, fill="#f3ead6", shadow={"dist": 10}, grain=0.08))
    S.append(sh("path", d="M880 270 C 920 230, 960 230, 990 260 S 1030 240, 1045 255", stroke="#2f6f73", weight=[3, 5], rotate=-4))
    S.append(sh("circle", cx=1000, cy=195, r=18, fill="none", stroke=CRAYON, weight=[3, 4], rotate=-4))
    # moonbeam or lamp light
    if night_end:
        S.append(sh("polygon", points=[[1290, 520], [1690, 520], [1500, 1080], [380, 1080]], fill="#bfd0ff", opacity=0.16, blend="screen",
                    outline=False, wobble=0, rough=0))
    else:
        S.append(sh("polygon", points=[[300, 470], [400, 470], [900, -40], [-80, -40]], fill=GOLD, opacity=0.1, blend="screen",
                    outline=False, wobble=0, rough=0))
        S.append(sh("circle", cx=330, cy=520, r=520, fill={"stops": [[0, "#ffcf6a"], [0.62, "#000000"], [1, "#000000"]], "gradient": "radial"},
                    blend="screen", outline=False, wobble=0, rough=0))
    # bedside table + lamp
    S.append(sh("rect", x=150, y=690, w=300, h=420, radius=10, fill="#7a3b2e", shade={"amount": 0.6, "offset": 0.1},
                texture={"style": "contour", "amount": 0.3}, shadow={"dist": 0, "dx": 30, "dy": 0, "opacity": 0.4}))
    S.append(sh("rect", x=130, y=670, w=340, h=34, radius=8, fill="#94503a", shade={"amount": 0.4}))
    S.append(sh("path", d="M270 670 C 250 640, 330 640, 310 600 L 290 520", stroke="#e8d7b6", weight=[6, 9]))
    S.append(sh("polygon", name="shade", points=[[200, 540], [400, 540], [360, 420], [240, 420]],
                fill=GOLD if not night_end else "#8a6a5a", shade={"amount": 0.35, "offset": 0.1}, texture={"style": "paper", "amount": 0.5}))
    # books on the table
    S.append(sh("rect", x=330, y=636, w=110, h=34, radius=4, fill="#2f6f73", shade={"amount": 0.4}))
    S.append(sh("rect", x=342, y=608, w=90, h=30, radius=4, fill="#c76a5a", shade={"amount": 0.4}))
    # headboard
    S.append(sh("path", d="M560 800 L 560 470 C 560 380, 700 360, 780 420 C 860 360, 1000 360, 1060 430 C 1110 370, 1240 380, 1260 470 L 1260 800 Z",
                fill="#7a3b2e", shade={"amount": 0.65, "offset": 0.12, "rim": 0.03}, texture={"style": "contour", "amount": 0.35}))
    # pillow
    S.append(sh("blob", cx=780, cy=640, rx=200, ry=80, lumpy=0.12, n=7, seed=5, fill="#e8dcc6",
                shade={"amount": 0.6, "offset": 0.15}))
    # child's head on pillow
    G.append({"name": "kid", "pivot": [760, 640], "cycle": {"style": "breathe", "amount": 1.5}})
    G.append({"name": "kidhead", "parent": "kid", "pivot": [760, 660], "lag": 0.1,
              "cycle": {"style": "nod", "speed": 0.35, "amount": 0.8} if not night_end else {"style": "breathe", "amount": 1.5}})
    G.append({"name": "kideyes", "parent": "kidhead", "pivot": [790, 600], "blink": {"every": 2.2, "seed": 4}})
    S.append(sh("blob", cx=740, cy=560, rx=110, ry=80, lumpy=0.25, n=9, seed=11, fill=HAIR_K, group="kidhead",
                shade={"amount": 0.4}))
    S.append(sh("circle", name="kidface", cx=780, cy=610, r=78, fill=SKIN, group="kidhead",
                shade={"amount": 0.8, "offset": 0.16, "rim": 0.05}))
    S.append(sh("blob", cx=760, cy=548, rx=92, ry=44, lumpy=0.3, n=7, seed=2, fill=HAIR_K, group="kidhead", clipTo="kidface"))
    S.append(sh("circle", cx=830, cy=640, r=16, fill="#e2716a", opacity=0.4, clipTo="kidface", outline=False, group="kidhead"))
    if night_end:
        S.append(sh("path", d="M786 612 q 12 10 24 0", stroke=INK, weight=[2, 4], group="kidhead"))
        S.append(sh("path", d="M826 612 q 10 9 20 0", stroke=INK, weight=[2, 4], group="kidhead"))
    else:
        S.append(sh("ellipse", cx=800, cy=608, rx=7, ry=10, fill=INK, outline=False, group="kideyes"))
        S.append(sh("ellipse", cx=840, cy=606, rx=6, ry=9, fill=INK, outline=False, group="kideyes"))
        S.append(sh("path", d="M806 650 q 14 12 28 0", stroke="#8a3a3a", weight=[2, 4], group="kidhead"))
    # quilt: patchwork hills (the story's country)
    ridge = "M470 1100 L 470 760 C 560 700, 640 690, 700 720 C 780 660, 880 650, 960 700 C 1040 640, 1160 650, 1230 700 C 1300 680, 1360 700, 1400 760 L 1400 1100 Z"
    G.append({"name": "quilt", "pivot": [930, 900], "cycle": {"style": "breathe", "amount": 1.2}})
    S.append(sh("path", name="quilt", d=ridge, fill=QUILT[4], shade={"amount": 0.6, "offset": 0.08, "rim": 0.03}, group="quilt",
                shadow={"dist": 0, "dx": 0, "dy": -18, "opacity": 0.35}))
    cols = [QUILT[i % 6] for i in (0, 2, 1, 3, 5, 4, 2, 0, 3, 1)]
    k = 0
    for row in range(3):
        for c in range(6):
            x0 = 470 + c * 160 + (row % 2) * 40
            y0 = 680 + row * 140
            S.append(sh("rect", x=x0, y=y0, w=150, h=130, fill=cols[k % len(cols)], clipTo="quilt", group="quilt",
                        texture={"style": ["hatch", "stipple", "riso", "halftone"][k % 4], "amount": 0.45, "scale": 8},
                        shade={"amount": 0.35}, wobble=0.6))
            k += 1
    # stitches
    S.append(sh("path", d="M480 860 C 700 820, 1000 830, 1390 870", stroke=CREAM, weight=[2, 3], gaps=0.6, group="quilt", opacity=0.7))
    S.append(sh("path", d="M480 990 C 760 960, 1000 970, 1390 1000", stroke=CREAM, weight=[2, 3], gaps=0.6, group="quilt", opacity=0.7))
    # parent sitting on the bed edge with the tablet (story beat only)
    if not night_end:
        G.append({"name": "mum", "pivot": [1330, 980], "cycle": {"style": "breathe", "amount": 1.1}})
        G.append({"name": "mumhead", "parent": "mum", "pivot": [1300, 640], "lag": 0.12, "rotate": -6,
                  "cycle": {"style": "nod", "speed": 0.5, "amount": 1.6}})
        G.append({"name": "mumeyes", "parent": "mumhead", "pivot": [1260, 560], "blink": {"every": 3.0, "seed": 9}})
        G.append({"name": "tablet", "parent": "mum", "pivot": [1140, 760], "lag": 0.16, "cycle": {"style": "idle", "amount": 0.8}})
        S.append(sh("path", d="M1230 1100 C 1210 900, 1230 720, 1330 660 C 1420 640, 1480 720, 1490 860 L 1520 1100 Z", fill="#c76a5a",
                    shade={"amount": 0.75, "offset": 0.16, "rim": 0.04}, texture={"style": "contour", "amount": 0.35}, group="mum"))
        S.append(sh("blob", cx=1320, cy=520, rx=96, ry=96, lumpy=0.2, n=9, seed=7, fill=HAIR_P, group="mumhead", shade={"amount": 0.5}))
        S.append(sh("circle", cx=1370, cy=440, r=40, fill=HAIR_P, group="mumhead", shade={"amount": 0.5}))
        S.append(sh("circle", name="mumface", cx=1280, cy=568, r=80, fill=SKIN2, group="mumhead", shade={"amount": 0.85, "offset": 0.18, "rim": 0.05}))
        S.append(sh("blob", cx=1330, cy=520, rx=80, ry=60, lumpy=0.2, n=7, seed=8, fill=HAIR_P, group="mumhead", clipTo="mumface"))
        S.append(sh("path", d="M1226 580 q 10 8 20 0", stroke=INK, weight=[2, 4], group="mumeyes"))
        S.append(sh("path", d="M1230 616 q 14 8 26 -2", stroke="#7a2a2a", weight=[2, 4], group="mumhead"))
        # tablet glow on faces + arm
        S.append(sh("circle", cx=1120, cy=700, r=300, fill={"stops": [[0, "#9fd4ff"], [0.62, "#000000"], [1, "#000000"]], "gradient": "radial"},
                    blend="screen", outline=False, group="tablet"))
        S.append(sh("rect", cx=1120, cy=740, w=200, h=140, radius=14, rotate=-14, fill="#1a1d3a", group="tablet", shade={"amount": 0.3}))
        S.append(sh("rect", cx=1120, cy=740, w=176, h=116, radius=8, rotate=-14, fill={"stops": [[0, "#ffe8b0"], [1, "#9fd4ff"]], "angle": 60, "steps": 3},
                    group="tablet", outline=False))
        S.append(sh("circle", cx=1100, cy=744, r=22, fill=BUNNY, group="tablet", outline=False))
        S.append(sh("ellipse", cx=1094, cy=708, rx=6, ry=20, rotate=-14, fill=BUNNY, group="tablet", outline=False))
        S.append(sh("path", d="M1330 760 C 1280 820, 1200 800, 1170 770", stroke="#c76a5a", weight=[22, 30], group="mum"))
    return illo(S, G, light={"angle": 210 if not night_end else 320, "rim": 30, "color": "#ffd59a" if not night_end else "#cfe0ff",
                             "shadow": "#2b1f5c", "strength": 1.0})


def text(t, voice, size, x, y, start, end=None, enter=None, anchor="topLeft", rotation=0, **k):
    d = dict(voice)
    d.update({"text": t, "size": size, "x": x, "y": y, "anchor": anchor, "start": start})
    if end is not None:
        d["end"] = end
    if enter:
        d["enter"] = enter
    if rotation:
        d["rotation"] = rotation
    d.setdefault("shadow", {"x": 3, "y": 5, "blur": 5, "color": "#06081c99"})
    d.update(k)
    return d



def fg_tufts():
    """Foreground grass and flower silhouettes at the frame edges, nearer than everything (parallax)."""
    S, G = [], []
    blades = []
    for side, x0, sgn in (("L", -40, 1), ("R", 1960, -1)):
        G.append({"name": "tuft" + side, "pivot": [x0 + sgn * 120, 1100], "sway": {"rotate": 2.5, "period": 2.6, "phase": 0.3 if sgn > 0 else 0.0}})
        for i in range(9):
            bx = x0 + sgn * (i * 34)
            h = 260 + (i * 53) % 220
            lean = sgn * (40 + (i * 29) % 90)
            col = ["#1c1234", "#2a1830", "#241a3e"][i % 3]
            S.append(sh("path", d=f"M{bx - 16} 1110 Q {bx + lean * 0.4} {1100 - h * 0.6} {bx + lean} {1100 - h} Q {bx + lean * 0.3 + 6} {1100 - h * 0.5} {bx + 18} 1110 Z",
                        fill=col, outline=False, group="tuft" + side, shade={"amount": 0.3}))
        fx = x0 + sgn * 170
        S.append(sh("path", points=[[fx, 1110], [fx + sgn * 30, 900], [fx + sgn * 20, 760]], smooth=True, stroke="#2a1830", weight=[5, 8], group="tuft" + side))
        S.append(sh("star", cx=fx + sgn * 20, cy=748, r=34, n=6, inner=0.5, fill="#c76a5a" if sgn > 0 else "#d39a3c", group="tuft" + side,
                    shade={"amount": 0.5}))
        S.append(sh("circle", cx=fx + sgn * 20, cy=748, r=10, fill=GOLD, group="tuft" + side, outline=False))
    L = illo(S, G)
    L["depth"] = -0.4
    return L


CRAYON_BUNNY = [  # bunny-local coordinates, feet at (0, 0), same proportions as bunny()
    [(-42, -2), (-82, -21), (-76, -132), (0, -143), (74, -139), (82, -15), (34, 0)],
    [(34, -114), (-19, -124), (-19, -208), (34, -213), (86, -210), (91, -124), (42, -114)],
    [(13, -202), (-11, -263), (-6, -310), (13, -309), (34, -303), (34, -242), (25, -206)],
    [(46, -206), (46, -263), (61, -303), (78, -299), (93, -284), (76, -232), (59, -202)],
    [(-63, -51), (-82, -63), (-78, -80), (-61, -80)],
]


def crayon_world(bx, by, s):
    """The child's crayon drawing of the scene (hills, stars and the bunny, drawn where the
    real bunny will stand); the papercut world fills in underneath it, then the lines fade."""
    def P(x, y):
        return f"{bx + s * x:.0f} {by + s * y:.0f}"
    paths = [
        {"d": "M-20 650 C 300 600, 620 640, 960 655 S 1600 610, 1940 640", "size": 9},
        {"d": "M-20 805 C 400 770, 900 830, 1300 795 S 1700 780, 1940 805", "size": 10},
    ]
    for pts in CRAYON_BUNNY:
        d = "M" + P(*pts[0])
        for i in range(1, len(pts) - 2, 3):
            d += " C " + ", ".join(P(*q) for q in pts[i:i + 3])
        paths.append({"d": d, "size": 13})
    paths.append({"d": f"M{P(57, -168)} L {P(61, -165)}", "size": 16})
    paths += [{"d": "M120 220 l 18 -40 l 18 40 l -42 -25 h 48 Z", "size": 5},
              {"d": "M860 120 l 12 -26 l 12 26 l -28 -16 h 32 Z", "size": 5}]
    return {"component": "sketch", "end": 1.3, "exit": {"preset": "fadeOut", "duration": 0.4},
            "props": {"draw": 0.45, "stagger": 0.07, "boil": 8, "color": CRAYON, "texture": "marker", "pressure": "even", "paths": paths}}

# ------------------------------------------------------------------ scenes
scenes = []

# 1. Bedroom: the story starts out loud. 0.0 - 3.8
scenes.append({
    "audio": [{"src": "sfx/scribble.wav", "at": 1.0, "volume": 0.45}],
    "id": "room", "duration": 3.8, "background": WALL,
    "camera": {"animate": [{"from": {"zoom": 1.06, "x": 40}, "to": {"zoom": 1.14, "x": -30, "y": 30}, "duration": 3.0, "ease": "inOutSine"},
                           {"to": {"zoom": 2.6, "x": -60, "y": 230}, "at": 2.65, "duration": 0.85, "ease": "inCubic"}]},
    "layers": [
        bedroom(False),
        text("Once upon\na time,", PARENT, "8.4vh", "9%", "12%", 0.25, end=3.2, exit="fadeOut", enter={"preset": "mask by line up", "duration": 0.9}, lineHeight=0.98),
        text("a bunny!", CHILD, "9vh", "27%", "31%", 1.0, end=3.2, exit="fadeOut", enter={"preset": "wipeIn", "duration": 0.55}, rotation=-7),
    ],
})

# 2. Quilt country: the quilt's hills become the story. 3.8 - 7.4
story_pal = ["#0f1640", "#3a3f86", "#3c4a8a", "#6b3a6e"]
walk = hops(-460, 330, 0.9, 3.4, 4, 60, s=1.4)
g2, s2 = bunny("b", 760, 990, 1.5, walk=walk)
moon_outline = illo([
    sh("circle", cx=1420, cy=250, r=120, fill="none", stroke=CRAYON, weight=[4, 7], gaps=0.45, boil=8,
       sway={"rotate": 4, "period": 6}),
], grain=0.0)
scenes.append({
    "id": "quilt", "duration": 3.6, "background": "#0f1640", "transition": {"type": "crossfade", "duration": 0.3},
    "camera": {"animate": [{"from": {"zoom": 1.12, "x": -80}, "to": {"zoom": 1.04, "x": 35}, "duration": 3.6, "ease": "outCubic"}]},
    "layers": [
        {"enter": {"preset": "fadeIn", "delay": 0.45, "duration": 0.7}, "component": "landscape", "props": {"style": "papercut", "time": "night", "terrain": ["hills", "hills", "woods"], "layers": 3,
                                              "horizon": 0.58, "depth": 0.3, "mist": 0.35, "parallax": 26, "sun": [1.6, -0.5], "seed": 4,
                                              "palette": ["#0f1640", "#3a3f86", "#3c4a8a", "#2f6f73"],
                                              "ranges": [{"color": "#3c4a8a"}, {"color": "#6b3a6e"}, {"color": "#2f6f73"}]}, "depth": 2},
        moon_outline,
        text("who lost\nthe", PARENT, "12vh", "7%", "12%", 0.15, enter={"preset": "mask by line up", "duration": 0.9}, lineHeight=0.95),
        text("moon.", dict(PARENT, color=GOLD), "12vh", "17.6%", "25.6%", 0.45, enter={"preset": "mask by line up", "duration": 0.7},
             animate=[{"to": {"rotation": -8}, "at": 1.5, "duration": 0.25, "ease": "outCubic"},
                      {"to": {"y": "+1000", "rotation": 70}, "at": 1.85, "duration": 1.1, "ease": "inCubic"}]),
        {"enter": {"preset": "fadeIn", "delay": 0.45, "duration": 0.7}, "component": "landscape", "props": {"style": "papercut", "time": "night", "terrain": ["hills", "dunes"], "layers": 2, "sky": False,
                                              "horizon": 0.74, "depth": 0.95, "parallax": 60, "seed": 9,
                                              "palette": ["#0f1640", "#3a3f86", "#c76a5a", "#8a4f3a"],
                                              "ranges": [{"color": "#c76a5a"}, {"color": "#d39a3c"}]}},
        {"enter": {"preset": "fadeIn", "delay": 0.45, "duration": 0.7}, "component": "plants", "props": {"style": "papercut", "time": "night", "species": ["grass", "flowers"], "count": 14, "spread": [0, 1],
                                           "ground": 0.99, "size": 160, "wind": 0.5, "onTwos": True,
                                           "colors": {"foliage": ["#5a2f4e", "#8a4f3a", "#c76a5a"], "trunk": "#3a1f2e", "bark": "#3a1f2e", "accent": GOLD}}},
        crayon_world(300, 990, 1.5),
        dict(illo(s2, g2), enter={"preset": "fadeIn", "delay": 0.7, "duration": 0.25}),
        fg_tufts(),
    ],
    "audio": [{"src": "sfx/scribble.wav", "at": 0.0, "volume": 0.5}, {"src": "sfx/scribble.wav", "at": 0.45, "volume": 0.35},
              {"sfx": "swoosh", "at": 1.8, "volume": 0.18}]
             + [{"src": "sfx/thump.wav", "at": round(0.9 + (i + 0.78) * 0.625, 3), "volume": 0.45} for i in range(4)],
})

# 3. The pond: the moon is in the water. 7.4 - 11.0
g3, s3 = bunny("c", 360, 1150, 2.0, walk=hops(-640, 0, 0.0, 1.3, 2, 60, s=1.5, hold=3.4), look_down=True)
refl = illo([
    sh("ellipse", cx=1180, cy=890, rx=160, ry=46, fill={"stops": [[0, "#fff0b8"], [0.62, "#000000"], [1, "#000000"]], "gradient": "radial"},
       opacity=0.85, blend="screen", outline=False, sway={"x": 8, "scale": 0.03, "period": 1.6}),
    sh("path", d="M1080 890 q 50 -10 100 0 t 100 0", stroke="#0f1640", weight=[2, 4], opacity=0.6, sway={"x": 14, "period": 1.1}),
    sh("path", d="M1110 910 q 40 -8 80 0 t 80 0", stroke="#0f1640", weight=[2, 3], opacity=0.5, sway={"x": -10, "period": 1.4}),
], grain=0.05)
arrow = {"component": "brush", "props": {"d": "M1500 330 C 1560 520, 1470 690, 1330 810 M1330 810 l 56 -2 M1330 810 l 10 -54",
                                         "texture": "marker", "color": CRAYON, "size": 14, "pressure": "even", "draw": 0.7, "delay": 0.5, "boil": 8}}
s3.insert(0, sh("path", d="M-60 1100 L -60 960 C 200 930, 520 960, 700 1010 C 780 1040, 820 1080, 840 1100 Z", fill="#3a2142",
                  shade={"amount": 0.5, "offset": 0.1, "rim": 0.02}, texture={"style": "contour", "amount": 0.3}))
scenes.append({
    "id": "pond", "duration": 3.4, "background": "#0f1640",
    "camera": {"animate": [{"from": {"zoom": 1.05, "x": 40}, "to": {"zoom": 1.12, "x": -40, "y": 30}, "duration": 3.6, "ease": "inOutSine"}]},
    "layers": [
        {"component": "landscape", "props": {"style": "papercut", "time": "night", "terrain": ["mountains", "woods"], "layers": 3,
                                              "horizon": 0.55, "depth": 0.12, "mist": 0.5, "parallax": 14, "sun": [1.6, -0.5], "seed": 12,
                                              "palette": ["#0b1238", "#2e3577", "#3c4a8a", "#2f6f73"],
                                              "ranges": [{"color": "#3c4a8a"}, {"color": "#6b3a6e"}, {"color": "#28505a"}]}, "depth": 2},
        {"component": "water", "props": {"style": "papercut", "time": "night", "sun": 0.62, "onTwos": True,
                                          "colors": {"top": "#22306a", "deep": "#121a46", "wave": "#3c4a8a", "glint": "#ffe3a0"}},
         "x": 960, "y": 860, "width": 1920, "height": 440},
        refl,
        text("moon.", dict(PARENT, color="#fff3cf"), "10vh", 1185, 872, 0.0, anchor="center", rotation=7, loop="float", scaleY=0.8, opacity=0.88,
             shadow={"x": 4, "y": 6, "blur": 8, "color": "#0a0f30cc"}),
        {"component": "plants", "props": {"style": "papercut", "time": "night", "species": ["grass", "bush"], "count": 7, "spread": [0, 0.45],
                                           "ground": 0.97, "size": 300, "fill": True, "wind": 0.45, "onTwos": True,
                                           "colors": {"foliage": ["#3a2142", "#6b3a6e", "#8a4f3a"], "trunk": "#2a1830", "bark": "#2a1830", "accent": GOLD}}},
        illo(s3, g3),
        arrow,
        text("It fell in\nthe pond!", CHILD, "7.6vh", "64%", "8%", 0.2, enter={"preset": "wipeIn", "duration": 0.6}, rotation=5, lineHeight=0.85),
    ],
    "audio": [{"src": "sfx/scribble.wav", "at": 0.2, "volume": 0.45}, {"src": "sfx/thump.wav", "at": 0.507, "volume": 0.45},
              {"src": "sfx/thump.wav", "at": 1.157, "volume": 0.45}, {"sfx": "ding", "at": 1.4, "volume": 0.12}],
})

# 4. Close-up: fishing the moon out. Type = two voices in one line. 11.0 - 14.2
g4, s4 = bunny("d", 560, 1010, 2.1, lantern=False,
               keys=[{"t": 0, "squash": 1}, {"t": 0.5, "squash": 0.9}, {"t": 1.05, "squash": 1.08, "ease": "outBack"}, {"t": 1.6, "squash": 1.0},
                     {"t": 2.0, "squash": 1.0}, {"t": 2.15, "squash": 0.86, "ease": "out"}, {"t": 2.4, "y": -90, "squash": 1.1, "ease": "out"},
                     {"t": 2.65, "y": 0, "squash": 1.04, "ease": "in"}, {"t": 2.75, "squash": 0.9, "ease": "out"}, {"t": 2.95, "squash": 1.0, "ease": "out"}],
               net=[{"t": 0, "rotate": 30}, {"t": 0.5, "rotate": 38}, {"t": 1.1, "rotate": -26, "ease": "outBack"}, {"t": 3.2, "rotate": -22}])
moon_up = illo([
    sh("circle", cx=1260, cy=560, r=420, fill={"stops": [[0, "#ffe7a6"], [0.62, "#000000"], [1, "#000000"]], "gradient": "radial"}, blend="screen", outline=False,
       animate={"keys": [{"t": 0, "opacity": 0.15, "y": 300}, {"t": 0.6, "opacity": 0.2, "y": 300}, {"t": 1.6, "opacity": 1, "y": 0, "ease": "out"}]}),
    sh("circle", name="mcut4", cx=1336, cy=506, r=153, visible=False),
    sh("circle", cx=1260, cy=560, r=170, fill="#ffe3a0", shade={"amount": 0.4, "offset": 0.08, "rim": 0.03},
       texture={"style": "halftone", "amount": 0.45, "scale": 9}, cut=["mcut4"],
       animate={"keys": [{"t": 0, "opacity": 0}, {"t": 1.55, "opacity": 0}, {"t": 1.75, "opacity": 1, "ease": "out"}]}),
    # drips
    sh("ellipse", cx=1200, cy=760, rx=9, ry=14, fill="#9fd4ff", outline=False,
       animate={"keys": [{"t": 0, "opacity": 0}, {"t": 1.4, "opacity": 0, "y": -60}, {"t": 1.5, "opacity": 1, "y": -60}, {"t": 2.3, "y": 200, "opacity": 0, "ease": "in"}]}),
    sh("ellipse", cx=1320, cy=780, rx=7, ry=11, fill="#9fd4ff", outline=False,
       animate={"keys": [{"t": 0, "opacity": 0}, {"t": 1.6, "opacity": 0, "y": -60}, {"t": 1.7, "opacity": 1, "y": -60}, {"t": 2.6, "y": 220, "opacity": 0, "ease": "in"}]}),
], grain=0.05)
scenes.append({
    "id": "catch", "duration": 3.0, "background": "#101845",
    "camera": {"animate": [{"from": {"zoom": 1.08, "y": 40}, "to": {"zoom": 1.03, "y": -8}, "duration": 1.3, "ease": "outCubic"},
                           {"to": {"zoom": 1.14, "x": 60, "y": -20}, "at": 1.62, "duration": 0.35, "ease": "outExpo"},
                           {"to": {"zoom": 1.06, "x": 20, "y": -10}, "at": 2.0, "duration": 1.2, "ease": "inOutSine"}],
               "shake": {"amount": 6, "at": 1.65, "duration": 0.3}},
    "layers": [
        {"component": "water", "props": {"style": "papercut", "time": "night", "sun": 0.66, "onTwos": True, "bands": 4,
                                          "colors": {"top": "#2a3a7a", "deep": "#121a46", "wave": "#4a5aa0", "glint": "#ffe3a0"}},
         "x": 960, "y": 820, "width": 1920, "height": 560},
        moon_up,
        {"component": "plants", "props": {"style": "papercut", "time": "night", "species": ["grass"], "count": 9, "spread": [0, 0.35],
                                           "ground": 1.0, "size": 420, "wind": 0.5, "onTwos": True,
                                           "colors": {"foliage": ["#2a1830", "#3a2142", "#6b3a6e"], "trunk": "#2a1830", "bark": "#2a1830", "accent": GOLD}}},
        illo(s4, g4),
        text("moon.", dict(PARENT, color="#fff3cf"), "15vh", 1250, 560, 0.0, anchor="center", end=1.8, exit={"preset": "fadeOut", "duration": 0.3},
             animate=[{"from": {"y": "+330", "rotation": 14, "opacity": 0.5}, "to": {"y": 560, "rotation": -4, "opacity": 1}, "at": 0.45, "duration": 0.95, "ease": "outBack"},
                      {"to": {"scale": 1.15}, "at": 1.55, "duration": 0.4, "ease": "outCubic"}],
             shadow={"x": 0, "y": 0, "blur": 30, "color": "#ffcf6aaa"}),
        text("Dreamed up", PARENT, "9vh", "56%", "9%", 0.05, enter={"preset": "mask by line up", "duration": 0.7}),
        text("together,", CHILD, "9vh", "62%", "21%", 0.25, enter={"preset": "wipeIn", "duration": 0.4}, rotation=-5),
    ],
    "audio": [{"src": "sfx/splash.wav", "at": 0.85, "volume": 0.6}, {"sfx": "chime", "at": 1.7, "volume": 0.3},
              {"src": "sfx/thump.wav", "at": 2.65, "volume": 0.5}],
})

# 5. The moon goes back up; the story is read aloud, word by word. 14.2 - 16.9
g5, s5 = bunny("e", 640, 930, 0.95, lantern=False, keys=[{"t": 0, "squash": 1.0}, {"t": 0.25, "squash": 0.9, "ease": "out"},
                                                         {"t": 0.6, "squash": 1.06, "y": -40, "ease": "out"}, {"t": 0.95, "squash": 1.0, "y": 0, "ease": "in"},
                                                         {"t": 1.05, "squash": 0.94}, {"t": 1.25, "squash": 1.0}])
# the hill she stands on, nearest to camera
s5.insert(0, sh("path", d="M-60 1100 L -60 900 C 240 860, 560 870, 900 930 C 1200 980, 1500 1020, 1980 1000 L 1980 1100 Z", fill="#2a1830",
                shade={"amount": 0.4, "offset": 0.06, "rim": 0.02}, texture={"style": "contour", "amount": 0.25}))
PAW = (640 + 0.95 * 30, 930 - 0.95 * 92)
moonkeys = [{"t": 0, "x": -470, "y": 380}, {"t": 0.2, "x": -470, "y": 380}, {"t": 1.3, "x": 0, "y": 0, "ease": "out"}]
moon5 = illo([
    sh("circle", cx=1520, cy=290, r=300, fill={"stops": [[0, "#ffe7a6"], [0.62, "#000000"], [1, "#000000"]], "gradient": "radial"}, blend="screen",
       outline=False, animate={"keys": moonkeys}),
    sh("path", d=f"M{PAW[0]:.0f} {PAW[1]:.0f} Q 880 760 1050 690", stroke=CREAM, weight=[1.5, 2.5], opacity=0.85,
       animate={"keys": [{"t": 0, "d": f"M{PAW[0]:.0f} {PAW[1]:.0f} Q 880 760 1050 690"}, {"t": 0.2, "d": f"M{PAW[0]:.0f} {PAW[1]:.0f} Q 880 760 1050 690"},
                         {"t": 1.3, "d": f"M{PAW[0]:.0f} {PAW[1]:.0f} L 1490 372", "ease": "out"}]}),
    sh("circle", name="mcut5", cx=1555, cy=265, r=70, visible=False, animate={"keys": moonkeys}),
    sh("circle", cx=1520, cy=290, r=78, fill="#ffe3a0", cut=["mcut5"], shade={"amount": 0.3, "offset": 0.08},
       texture={"style": "halftone", "amount": 0.35, "scale": 7}, animate={"keys": moonkeys}),
], grain=0.04)
scenes.append({
    "audio": [{"src": "sfx/pluck.wav", "at": 0.25, "volume": 0.45}, {"src": "sfx/thump.wav", "at": 0.95, "volume": 0.35}],
    "id": "sky", "duration": 3.1, "background": "#141c50", "transition": {"type": "crossfade", "duration": 0.5},
    "camera": {"animate": [{"from": {"zoom": 1.12, "y": 60}, "to": {"zoom": 1.03, "y": -10}, "duration": 2.7, "ease": "inOutSine"}]},
    "layers": [
        {"component": "landscape", "props": {"style": "papercut", "time": "night", "terrain": ["hills", "hills", "woods"], "layers": 3,
                                              "horizon": 0.62, "depth": 0.5, "mist": 0.3, "parallax": 10, "sun": [1.6, -0.5], "seed": 4,
                                              "palette": ["#141c50", "#3a4790", "#3c4a8a", "#2f6f73"],
                                              "ranges": [{"color": "#3c4a8a"}, {"color": "#6b3a6e"}, {"color": "#c76a5a"}]}, "depth": 1.5},
        moon5,
        illo(s5, g5),
        dict(PARENT, text="and read aloud, word by word.", size="6.4vh", x=1072, y=548, anchor="center", rotation=-29.8, start=0.45,
             split="words", enter={"preset": "fadeUp", "duration": 0.25}, stagger=0.1, shadow={"x": 3, "y": 5, "blur": 5, "color": "#06081c99"}),
    ],
})

# 6. Back in the room: the moon is in the window. The child is asleep. 16.9 - 20.0
scenes.append({
    "audio": [{"src": "sfx/scribble.wav", "at": 0.6, "volume": 0.45}],
    "id": "end", "duration": 3.1, "background": WALL, "transition": {"type": "crossfade", "duration": 0.6},
    "camera": {"animate": [{"from": {"zoom": 1.35, "x": 182, "y": -57}, "to": {"zoom": 1.0, "x": 0, "y": 0}, "duration": 2.2, "ease": "inOutCubic"}]},
    "layers": [
        bedroom(True),
        text("the end.", CHILD, "10vh", "6%", "8%", 0.35, enter={"preset": "wipeIn", "duration": 0.45}, rotation=-5),
        text("bedtime stories you make together,\nread aloud every night", dict(PARENT, axes={"SOFT": 100, "WONK": 1, "opsz": 36, "wght": 400}), "3.9vh", "6.4%", "25%", 0.9,
             enter={"preset": "mask by line up", "duration": 0.6}, lineHeight=1.2, italic=True),
        # the wordmark is sewn into the quilt the whole story came out of
        text("Burrow", dict(PARENT, axes={"SOFT": 100, "WONK": 1, "opsz": 144, "wght": 640}), "15vh", 935, 900, 1.3, anchor="center", rotation=-3,
             enter={"preset": "mask by line up", "duration": 0.7}, letterSpacing="-0.01em", color="#f6e6c4",
             shadow={"x": 0, "y": 3, "blur": 2, "color": "#2a1830cc"}),
    ],
})

for sc in scenes:
    out = []
    for L in sc["layers"]:
        out.extend(lit(L) if L.get("component") == "illo" else [L])
    sc["layers"] = out

spec = {
    "size": "16:9", "fps": 24, "encode": {"crf": 25},
    "intent": {"pace": "calm", "audience": "parents of 3-7 year olds", "tone": "warm, hushed, wonder"},
    "post": {"bloom": 0.14, "bloomThreshold": 0.72, "halation": 0.18, "vignette": 0.3, "grain": 0.018, "grainSize": 1.2},
    "music": {
        "bpm": 72, "meter": "3/4", "key": "F", "end": "ring", "master": {"lufs": -15, "reverb": "hall"},
        "tracks": [
            {"inst": "harp", "chords": "F Dm Bb C F Dm Bb F", "play": "arp/8", "gain": -7, "reverb": 0.5, "pan": -0.2},
            {"inst": "strings", "chords": "F Dm Bb C F Dm Bb F", "play": "pad", "gain": -15, "reverb": 0.6, "at": 2.5},
            {"inst": "cello", "notes": "o2 F2/2. | D2/2. | Bb1/2. | C2/2. | F2/2. | D2/2. | Bb1/2. | F2/2.", "gain": -13, "reverb": 0.5, "at": 5},
            {"inst": "celesta", "notes": "o5 A4/4 C5/8 A4/8 F4/4 | A4/4 D5/8 C5/8 A4/4 | Bb4/4. A4/8 G4/4 | C5/8! D5/8 F5/8 G5/8 A5/8 C6/8! | A5/4. r/4. | r/2. | D5/4. C5/8 Bb4/4 | [F4 A4 C5 F5]/2.",
             "gain": -4, "reverb": 0.55, "pan": 0.15},
            {"inst": "glockenspiel", "notes": "C5/16 D5/16 F5/16 G5/16 A5/16 C6/16 | A5/16 G5/16 F5/16 D5/16 F5/16 G5/16 A5/16 C6/16 D6/16 C6/16 A5/16 F5/16",
             "at": 11.25, "gain": -9, "reverb": 0.6, "pan": 0.25},
            {"inst": "timpani", "notes": "o2 r/4 F2/16 F2/16 F2/16 F2/16 C2/16 C2/16 C2/16 C2/16 | F2/2.", "at": 10.0, "gain": -9, "reverb": 0.5},
            {"inst": "choir", "chords": "Bb C F Dm", "play": "pad", "at": 10.0, "gain": -15, "reverb": 0.7},
            {"inst": "french-horn", "notes": "o3 [F3 A3 C4]/2. | [D3 F3 A3]/2.", "at": 12.5, "gain": -13, "reverb": 0.6},
        ],
    },
    "audio": [{"src": "sfx/night.wav", "volume": 0.35, "fadeIn": 1.0, "fadeOut": 1.5}],
    "scenes": scenes,
}

with open(os.path.join(HERE, "spec.json"), "w") as f:
    json.dump(spec, f, separators=(",", ":"))
print("spec bytes", os.path.getsize(os.path.join(HERE, "spec.json")))
