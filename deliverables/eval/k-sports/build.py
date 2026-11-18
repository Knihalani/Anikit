#!/usr/bin/env python3
"""Builds spec.json for the Northbridge City v Harbour Athletic opener.

Idea: each club is drawn from its own city's curve -- Northbridge's steel
through-arch bridge, Harbour's curved stone breakwater with its lighthouse.
Each is half a circle. At the peak they slam together and the camera pulls
back: the two halves are the centre circle of tonight's floodlit pitch.
Python only computes the bridge/breakwater geometry (path strings).
"""
import json, math

INK = "#0A0E14"
CHALK = "#F4F2EA"
COBALT = "#1D3FE6"
COBALT_LIT = "#4C74FF"
ORANGE = "#FF5A1F"
ORANGE_LIT = "#FF7438"
FONT = "Big Shoulders"


def f(v):
    return f"{v:.1f}"


def arc_pts(cx, cy, r, a0, a1, n):
    return [(cx + r * math.cos(math.radians(a0 + (a1 - a0) * i / n)),
             cy - r * math.sin(math.radians(a0 + (a1 - a0) * i / n))) for i in range(n + 1)]


def poly(pts):
    return "M" + " L".join(f"{f(x)} {f(y)}" for x, y in pts)


def bbox(paths):
    xs, ys = [], []
    for d in paths:
        nums = [float(v) for v in d.replace("M", " ").replace("L", " ").split()]
        xs += nums[0::2]
        ys += nums[1::2]
    return min(xs), min(ys), max(xs), max(ys)


def placed(d, ox, oy, **kw):
    """A path layer whose d is in absolute frame coordinates (offset ox, oy)."""
    x0, y0, _, _ = bbox([d])
    layer = {"shape": "path", "d": d, "fill": "none", "x": ox + x0, "y": oy + y0, "anchor": "topLeft"}
    layer.update(kw)
    return layer


def bridge_parts(R, r_in, segs, deck_y_off=0.0, angle0=180, angle1=0):
    """Through-arch bridge around centre (0,0): outer chord, inner chord, Warren truss, hangers."""
    outer = arc_pts(0, 0, R, angle0, angle1, 96)
    inner = arc_pts(0, 0, r_in, angle0, angle1, 96)
    zig = []
    for i in range(segs + 1):
        a = angle0 + (angle1 - angle0) * i / segs
        rr = R if i % 2 == 0 else r_in
        zig.append((rr * math.cos(math.radians(a)), -rr * math.sin(math.radians(a))))
    hang = []
    for i in range(1, segs):
        a = angle0 + (angle1 - angle0) * i / segs
        x = r_in * math.cos(math.radians(a))
        y = -r_in * math.sin(math.radians(a))
        hang.append(f"M{f(x)} {f(y)} L{f(x)} {f(deck_y_off)}")
    return poly(outer), poly(inner), poly(zig), " ".join(hang)


def wave(x0, x1, y, amp, wl):
    pts, x = [], x0
    while x <= x1:
        pts.append((x, y + amp * math.sin((x - x0) / wl * 2 * math.pi)))
        x += 6
    return poly(pts)


def lights(times):
    """Floodlight switch-on: clunk, over-bright flash, settle."""
    anims = []
    for name, at in times:
        anims.append({"keyframes": {name: [0, 1.6, 0.55, 1.05, 1]}, "at": at, "duration": 0.32})
    return anims


# ---------------------------------------------------------------- scene 1: lights
S1 = {
    "duration": 2.0,
    "background": "#000000",
    "layers": [
        {"type": "shader", "src": "pitch.glsl", "id": "pitch1",
         "uniforms": {"pz": 1.32, "l1": 0, "l2": 0, "l3": 0, "l4": 0, "boards": 0},
         "animate": lights([("l1", 0.45), ("l2", 0.72), ("l3", 0.99), ("l4", 1.26)]) + [
             {"from": {"pz": 1.32}, "to": {"pz": 1.02}, "duration": 2.0, "ease": "outCubic"},
             {"from": {"boards": 0}, "to": {"boards": 1}, "at": 1.5, "duration": 0.12}]},
    ],
}

# ---------------------------------------------------------------- scene 2: Northbridge
CX, CY, R, RI = 960, 690, 540, 486
o, i_, z, h = bridge_parts(R, RI, 22)
S2 = {
    "duration": 2.0,
    "background": "radial-gradient(#2E55FF, #0A1A9A)",
    "camera": {"animate": [{"from": {"zoom": 1.0}, "to": {"zoom": 1.05}, "duration": 2.0, "ease": "linear"}]},
    "layers": [
        # floodlight raking in from the top-left corner
        {"shape": "rect", "width": 2600, "height": 700, "x": 560, "y": 120, "rotation": 28,
         "fill": "linear-gradient(90deg, rgba(255,255,255,0), rgba(220,230,255,0.22), rgba(255,255,255,0))",
         "blend": "screen", "blur": 30},
        # river under the deck: reflected hangers, broken by the current
        placed(h, CX, CY, scale=[1, -0.35], stroke={"color": CHALK, "width": 3, "dash": [10, 14]},
               opacity=0.0, animate=[{"to": {"opacity": 0.22}, "at": 0.5, "duration": 0.6}]),
        placed(f"M0 0 L1920 0", 0, CY, stroke={"color": CHALK, "width": 12, "cap": "butt"},
               trimEnd=0, animate=[{"to": {"trimEnd": 1}, "duration": 0.45, "ease": "inOutExpo"}]),
        placed(o, CX, CY, stroke={"color": CHALK, "width": 12, "cap": "round"}, trimEnd=0,
               animate=[{"to": {"trimEnd": 1}, "at": 0.05, "duration": 0.6, "ease": "outExpo"}]),
        placed(i_, CX, CY, stroke={"color": CHALK, "width": 5, "cap": "round"}, trimEnd=0,
               animate=[{"to": {"trimEnd": 1}, "at": 0.12, "duration": 0.6, "ease": "outExpo"}]),
        placed(z, CX, CY, stroke={"color": CHALK, "width": 3.5, "join": "miter"}, trimEnd=0,
               animate=[{"to": {"trimEnd": 1}, "at": 0.2, "duration": 0.7, "ease": "outCubic"}]),
        placed(h, CX, CY, stroke={"color": CHALK, "width": 2.5}, opacity=0,
               animate=[{"to": {"opacity": 0.9}, "at": 0.35, "duration": 0.25}]),
        {"text": "NORTHBRIDGE CITY", "font": FONT, "weight": 900, "size": 196, "color": CHALK,
         "letterSpacing": "-0.01em", "x": 960, "y": CY + 40, "anchor": "top",
         "enter": {"preset": "mask by line up", "delay": 0.22}},
    ],
}

# ---------------------------------------------------------------- scene 3: Harbour
HX, HY, HR = 960, 468, 496
wall = poly(arc_pts(0, 0, HR, 180, 360, 96))
wall_in = poly(arc_pts(0, 0, HR - 34, 180, 360, 96))
waves = []
for k, yy in enumerate([110, 190, 270, 350, 425]):
    half = math.sqrt(max(HR - 60, 1) ** 2 - yy ** 2) - 30 if yy < HR - 60 else 60
    waves.append((yy, wave(-half, half, yy, 7, 58)))
LX, LY = HX + HR, HY  # lighthouse on the breakwater's east end
S3 = {
    "duration": 2.0,
    "background": "radial-gradient(#FF7A3A, #D63E0C)",
    "camera": {"animate": [{"from": {"zoom": 1.05}, "to": {"zoom": 1.0}, "duration": 2.0, "ease": "linear"}]},
    "layers": [
        # the lighthouse beam sweeping the bay
        {"shape": "path", "d": "M0 160 L1800 0 L1800 320 Z", "fill":
            "linear-gradient(90deg, rgba(255,236,214,0.75), rgba(255,236,214,0))",
         "x": LX, "y": LY - 151, "anchor": [0, 0.5], "blend": "screen", "opacity": 0,
         "rotation": "=196 - 18*sin(t*1.9)",
         "animate": [{"to": {"opacity": 1}, "at": 0.55, "duration": 0.2}]},
        *[placed(d, HX, HY, stroke={"color": INK, "width": 4, "cap": "round"},
                 x="=%s + 18*sin(t*2.4 + %d)" % (f(HX + bbox([d])[0]), k), opacity=0,
                 animate=[{"to": {"opacity": 1}, "at": 0.3 + 0.06 * k, "duration": 0.3}])
          for k, (yy, d) in enumerate(waves)],
        placed(wall, HX, HY, stroke={"color": INK, "width": 34, "cap": "butt", "dash": [54, 7]}, trimEnd=0,
               animate=[{"to": {"trimEnd": 1}, "duration": 0.6, "ease": "outExpo"}]),
        placed(wall_in, HX, HY, stroke={"color": INK, "width": 4}, trimEnd=0,
               animate=[{"to": {"trimEnd": 1}, "at": 0.06, "duration": 0.6, "ease": "outExpo"}]),
        # lighthouse: tapered tower with bands, lantern
        *[dict(l, opacity=0, animate=[{"to": {"opacity": 1}, "at": 0.42 + 0.05 * n, "duration": 0.12}])
          for n, l in enumerate([
            {"shape": "path", "d": "M0 118 L7 0 L33 0 L40 118 Z", "fill": INK, "x": LX - 20, "y": LY - 135, "anchor": "topLeft"},
            {"shape": "rect", "width": 30, "height": 12, "fill": "#FFE2CC", "x": LX, "y": LY - 57},
            {"shape": "rect", "width": 26, "height": 12, "fill": "#FFE2CC", "x": LX, "y": LY - 97},
            {"shape": "rect", "width": 38, "height": 7, "fill": INK, "x": LX, "y": LY - 138},
            {"shape": "circle", "width": 24, "height": 24, "fill": "#FFF3E2", "x": LX, "y": LY - 151, "glow": "#FFE7CF"}])],
        {"text": "HARBOUR ATHLETIC", "font": FONT, "weight": 900, "size": 196, "color": INK,
         "letterSpacing": "-0.01em", "x": 960, "y": 100, "anchor": "top",
         "enter": {"preset": "mask by line up", "delay": 0.18}},
    ],
}

# ---------------------------------------------------------------- scene 4: the clash, the reveal
Z0, Z1, Z2, ZAT, ZDUR = 3.6, 1.35, 1.42, 0.62, 1.1
LOCK = 0.42
zoom_anim = [{"from": {"pz": Z0}, "to": {"pz": Z1}, "at": ZAT, "duration": ZDUR, "ease": "inOutExpo"},
             {"to": {"pz": Z2}, "at": ZAT + ZDUR, "duration": 4.0 - ZAT - ZDUR, "ease": "linear"}]
scale_anim = [{"from": {"scale": Z0}, "to": {"scale": Z1}, "at": ZAT, "duration": ZDUR, "ease": "inOutExpo"},
              {"to": {"scale": Z2}, "at": ZAT + ZDUR, "duration": 4.0 - ZAT - ZDUR, "ease": "linear"}]
CR = 141  # centre-circle radius in pitch px
# left half = the bridge, turned to face the halfway line
lo, li, lz, lh = bridge_parts(CR, CR - 13, 10, angle0=90, angle1=270)
lh = ""  # no hangers at this scale
# right half = the breakwater
ro = poly(arc_pts(0, 0, CR, 90, -90, 64))


def half(d, anchor, x_from, stroke, extra=None):
    layer = {"shape": "path", "d": d, "fill": "none", "x": 960, "y": 540, "anchor": anchor,
             "stroke": stroke, "scale": Z0,
             "animate": [{"from": {"x": x_from}, "to": {"x": 960}, "duration": LOCK, "ease": "inExpo"}] + scale_anim}
    if extra:
        layer.update(extra)
    return layer


S4 = {
    "duration": 4.0,
    "background": "#000000",
    "camera": {"shake": {"amount": 14, "at": LOCK, "duration": 0.35}},
    "layers": [
        {"type": "shader", "src": "pitch.glsl",
         "uniforms": {"pz": Z0, "l1": 1, "l2": 1, "l3": 1, "l4": 1, "boards": 1, "tint": 0, "dim": 0.35},
         "animate": zoom_anim + [
             {"to": {"tint": 0.32, "dim": 0.0}, "at": LOCK, "duration": 0.5, "ease": "outCubic"}]},
        # darkened centre so the time reads over the halfway line
        {"shape": "circle", "width": 2 * CR - 14, "height": 2 * CR - 14, "x": 960, "y": 540,
         "fill": "radial-gradient(rgba(4,8,6,0.88), rgba(4,8,6,0.72))", "scale": Z0, "opacity": 0,
         "animate": scale_anim + [{"to": {"opacity": 1}, "at": 1.5, "duration": 0.35}]},
        half(lo, "right", -400, {"color": COBALT_LIT, "width": 7, "cap": "butt"}),
        half(li, "right", -400, {"color": COBALT_LIT, "width": 2.5}),
        half(lz, "right", -400, {"color": COBALT_LIT, "width": 1.6}),
        half(ro, "left", 2320, {"color": ORANGE_LIT, "width": 9, "cap": "butt", "dash": [13, 2]}),
        # impact ring at the lock
        {"shape": "circle", "width": 2 * CR, "height": 2 * CR, "x": 960, "y": 540, "fill": "none",
         "stroke": {"color": CHALK, "width": 3}, "scale": Z0, "opacity": 0,
         "animate": [{"keyframes": {"opacity": [0, 1, 0]}, "at": LOCK, "duration": 0.45},
                     {"from": {"scale": Z0}, "to": {"scale": Z0 * 1.6}, "at": LOCK, "duration": 0.45, "ease": "outExpo"}]},
        {"shape": "rect", "width": 1920, "height": 1080, "fill": "#FFFFFF", "blend": "screen", "opacity": 0,
         "animate": [{"keyframes": {"opacity": [0, 0.55, 0]}, "at": LOCK - 0.02, "duration": 0.22}]},
        # the card
        {"text": "NORTHBRIDGE\nCITY", "font": FONT, "weight": 900, "size": 100, "lineHeight": 0.88,
         "color": CHALK, "align": "center", "x": 470, "y": 540, "anchor": "center",
         "enter": {"preset": "mask by line up", "delay": 1.55}},
        {"text": "HARBOUR\nATHLETIC", "font": FONT, "weight": 900, "size": 100, "lineHeight": 0.88,
         "color": CHALK, "align": "center", "x": 1450, "y": 540, "anchor": "center",
         "enter": {"preset": "mask by line up", "delay": 1.65}},
        {"text": "KICK-OFF", "font": FONT, "weight": 700, "size": 30, "letterSpacing": "0.14em",
         "color": CHALK, "x": 960, "y": 452, "anchor": "center", "enter": {"preset": "fadeUp", "delay": 1.75}},
        {"text": "19:45", "font": FONT, "weight": 900, "size": 150, "color": CHALK,
         "x": 960, "y": 542, "anchor": "center", "enter": {"preset": "mask by line up", "delay": 1.7}},
        {"text": "WOMEN'S SUPER LEAGUE", "font": FONT, "weight": 700, "size": 25, "letterSpacing": "0.08em",
         "color": CHALK, "x": 960, "y": 630, "anchor": "center", "enter": {"preset": "fadeUp", "delay": 1.85}},
    ],
}

spec = {
    "size": "16:9",
    "fps": 30,
    "intent": {"pace": "fast", "mood": "big-match night"},
    "music": {"style": "cinematic-hybrid", "bpm": 120, "key": "Dm", "mood": "dark epic",
              "hits": ["@2.0", "@4.0", "@6.42"], "end": "ring",
              "sfx": [{"sfx": "riser", "at": "@6.42"}]},
    "sound": {"palette": "cinematic", "ambience": {"type": "crowd", "level": -10}, "density": 0.45,
              "cues": [{"sfx": "metal", "at": t, "x": x, "gain": -2, "pitch": -7}
                       for t, x in [(0.45, -0.6), (0.72, 0.6), (0.99, 0.6), (1.26, -0.6)]] +
                      [{"sfx": "impact", "at": 6.0 + LOCK, "gain": 0},
                       {"sfx": "boom", "at": 6.0 + LOCK, "gain": -3}]},
    "post": {"preset": "cinematic", "bloom": 0.22, "halation": 0.15, "grain": 0.035, "vignette": 0.2},
    "scenes": [S1, S2, S3, S4],
}
json.dump(spec, open("spec.json", "w"), indent=1)
print("ok")
