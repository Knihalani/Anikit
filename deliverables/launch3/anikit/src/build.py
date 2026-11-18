#!/usr/bin/env python3
"""Builds spec.json for the Anikit Video launch film "The Drill".

A one-page drill chart (yellow dots on turf) is performed by a 144-piece marching band.
Everything on the field lives in one world group; each shot is a camera on that group
(scale/rotation/translation), mirrored by the turf shader's uniforms.
Units: 1 yard = 36 world px; midfield centre = (960, 540).
"""
import json, math, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, '..', 'spec.json')
Y = 36.0
BPM = 128
BAR = 240 / BPM          # 1.875 s
BEAT = BAR / 4
N = 144

def bar(b):              # start time of bar b (1-based)
    return (b - 1) * BAR

def W(p):                # yards -> world px
    return (960 + p[0] * Y, 540 + p[1] * Y)

# ---------------------------------------------------------------- formations (yards)
def block(yc, sp=1.75):
    return [((c - 5.5) * sp, (r - 5.5) * sp + yc) for r in range(12) for c in range(12)]

def rings(rot=(0, 0, 0)):
    pts = []
    for (r, n), a0 in zip(((14.0, 64), (10.0, 48), (6.0, 32)), rot):
        for k in range(n):
            a = math.radians(a0) + 2 * math.pi * (k + 0.5) / n
            pts.append((r * math.cos(a), r * math.sin(a)))
    return pts

def wave():
    pts = []
    for r in range(6):
        for c in range(24):
            x = (c - 11.5) * 1.5
            pts.append((x, (r - 2.5) * 2.4 + 2.2 * math.sin(x * 0.17 + r * 0.5) - 1.0))
    return pts

def company_front():
    return [((c - 23.5) * 1.27, (r - 1) * 1.6 + 2.5) for r in range(3) for c in range(48)]

def letters():
    w, gap, hgt = 8.0, 2.6, 12.0
    L = {
        'A': [((0, 12), (4, 0)), ((4, 0), (8, 12)), ((1.7, 7.4), (6.3, 7.4))],
        'N': [((0, 12), (0, 0)), ((0, 0), (8, 12)), ((8, 12), (8, 0))],
        'I': [((4, 0), (4, 12)), ((1.6, 0), (6.4, 0)), ((1.6, 12), (6.4, 12))],
        'K': [((0, 0), (0, 12)), ((7.6, 0), (0.6, 6.6)), ((2.9, 4.6), (8, 12))],
        'T': [((0, 0), (8, 0)), ((4, 0.9), (4, 12))],
    }
    word = 'ANIKIT'
    total_w = len(word) * w + (len(word) - 1) * gap
    segs = []
    for i, ch in enumerate(word):
        ox = -total_w / 2 + i * (w + gap)
        oy = -hgt / 2 - 2.0
        for (a, b) in L[ch]:
            segs.append(((a[0] + ox, a[1] + oy), (b[0] + ox, b[1] + oy)))
    lens = [math.dist(a, b) for a, b in segs]
    tot = sum(lens)
    raw = [l / tot * N for l in lens]
    cnt = [max(2, int(round(x))) for x in raw]
    while sum(cnt) > N:
        i = max(range(len(cnt)), key=lambda j: cnt[j] - raw[j]); cnt[i] -= 1
    while sum(cnt) < N:
        i = max(range(len(cnt)), key=lambda j: raw[j] - cnt[j]); cnt[i] += 1
    pts = []
    for (a, b), n in zip(segs, cnt):
        for k in range(n):
            f = (k + 0.5) / n
            pts.append((a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f))
    return pts

def hungarian(cost):
    n = len(cost); INF = float('inf')
    u = [0.0] * (n + 1); v = [0.0] * (n + 1); p = [0] * (n + 1); way = [0] * (n + 1)
    for i in range(1, n + 1):
        p[0] = i; j0 = 0; minv = [INF] * (n + 1); used = [False] * (n + 1)
        while True:
            used[j0] = True; i0 = p[j0]; delta = INF; j1 = 0
            row = cost[i0 - 1]
            for j in range(1, n + 1):
                if not used[j]:
                    cur = row[j - 1] - u[i0] - v[j]
                    if cur < minv[j]: minv[j] = cur; way[j] = j0
                    if minv[j] < delta: delta = minv[j]; j1 = j
            for j in range(n + 1):
                if used[j]: u[p[j]] += delta; v[j] -= delta
                else: minv[j] -= delta
            j0 = j1
            if p[j0] == 0: break
        while True:
            j1 = way[j0]; p[j0] = p[j1]; j0 = j1
            if j0 == 0: break
    ans = [0] * n
    for j in range(1, n + 1): ans[p[j] - 1] = j - 1
    return ans

def assign(prev, nxt):
    cost = [[math.dist(a, b) ** 2 for b in nxt] for a in prev]
    idx = hungarian(cost)
    return [nxt[i] for i in idx]

# ---------------------------------------------------------------- the drill (key times, seconds)
T_STOP0, T_STOP1 = bar(13), bar(14)
keys = []                       # list of (time, positions per marcher)
p = block(-11.0); keys.append((0.0, p))
p = block(-6.0);  keys.append((bar(3), p))
p = block(-2.0);  keys.append((bar(5), p))
# bars 5-6: the block becomes a gull; bars 7-10: it flies the length of the field, one wingbeat every 2 beats
from bird import bird
def gull(phase, cx):
    return [(-y + cx, x) for x, y in bird(phase, 0, 0, 1.0)]   # rotated to fly toward +x
X0, X1 = -14.0, 12.0
g0 = gull(0.0, X0)
order = hungarian([[math.dist(a, b) ** 2 for b in g0] for a in p])   # marcher i -> outline slot order[i]
for k in range(0, 65):
    f = k / 64
    pts = gull((k / 8) % 1.0, X0 + (X1 - X0) * f)
    keys.append((bar(7) + k * BEAT / 4, [pts[order[i]] for i in range(N)]))
p = keys[-1][1]
p = assign(p, company_front()); keys.append((bar(13), p))
keys.append((bar(14), p))
p = assign(p, [tuple(q) for q in json.load(open(os.path.join(HERE, 'anikit_outline.json')))]); keys.append((bar(15) + 2 * BEAT, p))
keys.append((30.5, p))

# march-step easing between sets is linear (it is marching); flows in the payoff ease
WIN = [0.0, 31.0]
def interp_expr(axis, i):
    ts, vs = [], []
    lo = max([k for k, (t, _) in enumerate(keys) if t <= WIN[0]] or [0])
    hi = min([k for k, (t, _) in enumerate(keys) if t >= WIN[1]] or [len(keys) - 1])
    for t, pts in keys[lo:hi + 1]:
        ts.append(round(t, 4)); vs.append(round(W(pts[i])[axis], 1))
    return "=interp([%s],[%s],gt)" % (','.join(map(str, ts)), ','.join(map(str, vs)))

# ---------------------------------------------------------------- sections and the look
# rows 0-1 of the starting block: colour guard (24); rows 10-11: percussion (24); else brass (96)
def role(i):
    r = i // 12
    return 'guard' if r < 2 else ('perc' if r >= 10 else 'brass')

DAY = dict(coat="radial-gradient(#D8413C, #8A1420)", hat="radial-gradient(#3B3D49, #0B0C12)",
           plume="#F6F3EA", bell="radial-gradient(#FFF0B8, #C08A26)", head="#EFEBE1",
           shadow="#0B2209", sh_op=0.46)
NIGHT = dict(coat="radial-gradient(#C9384A, #4E0B1C)", hat="radial-gradient(#4A5060, #07080D)",
             plume="#EEF2FA", bell="radial-gradient(#F2F6FF, #9B7A3A)", head="#E4E8EF",
             shadow="#03070A", sh_op=0.30)

MOVE = "(gt < %.4f || gt > %.4f)" % (T_STOP0 + 0.02, T_STOP1)
BOB = "=1 + 0.06 * abs(sin(beat * 3.14159)) * if(%s, 1, 0)" % MOVE
SWAY = " + 2.4 * sin(beat * 3.14159) * if(%s, 1, 0)" % MOVE

def marcher_layers(i, L, night):
    X = interp_expr(0, i); Yx = interp_expr(1, i)
    def at(dx, dy):
        return (X if dx == 0 else X + " + %g" % dx), (Yx if dy == 0 else Yx + " + %g" % dy)
    out = []
    rl = role(i)
    if not night and os.environ.get("STYLE") != "overcast":
        # one long shadow, low sun from the upper left
        sx, sy = at(30, 22)
        out.append({"shape": "rect", "width": 82, "height": 15, "radius": 7.5, "fill": L["shadow"],
                    "opacity": L["sh_op"], "rotation": 36.9, "x": sx, "y": sy})
        if rl == 'guard':
            fx, fy = at(58, 30)
            out.append({"shape": "rect", "width": 50, "height": 22, "radius": 3, "fill": L["shadow"],
                        "opacity": L["sh_op"] * 0.8, "rotation": 36.9, "x": fx, "y": fy})
    else:
        for ang, dx, dy in ((45, 12, 12), (-45, 12, -12), (135, -12, 12), (-135, -12, -12)):
            sx, sy = at(dx, dy)
            out.append({"shape": "rect", "width": 36, "height": 13, "radius": 6.5, "fill": L["shadow"],
                        "opacity": L["sh_op"], "rotation": ang, "x": sx, "y": sy})
    # instrument / silk (in front of the body: the band faces +y, toward the home side)
    if rl == 'brass':
        bx, by = at(0, 15)
        out.append({"shape": "circle", "width": 13, "height": 13, "fill": L["bell"], "x": bx, "y": by})
    elif rl == 'perc':
        bx, by = at(0, 15)
        out.append({"shape": "circle", "width": 17, "height": 17, "fill": L["head"], "stroke": "#8F8A7E",
                    "strokeWidth": 1.5, "x": bx, "y": by})
    # body (shoulders), shako, plume
    import random
    rnd = random.Random(i * 7919)
    out.append({"shape": "circle", "width": 29, "height": 17, "fill": L["coat"], "x": X + SWAY, "y": Yx, "scale": BOB,
                "rotation": round(rnd.uniform(-8, 8), 1)})
    # white cross-belt across the shoulders
    out.append({"shape": "rect", "width": 22, "height": 3, "fill": "#EDE8DC", "opacity": 0.85, "x": X + SWAY, "y": Yx + " + 1",
                "rotation": round(rnd.uniform(18, 26), 1)})
    hx, hy = at(0, -1)
    out.append({"shape": "circle", "width": 13, "height": 13, "fill": L["hat"], "x": hx + SWAY, "y": hy, "scale": BOB})
    pdx, pdy = round(rnd.uniform(-3, -1), 1), round(rnd.uniform(-5, -3), 1)
    px, py = at(pdx, pdy)
    out.append({"shape": "circle", "width": 8, "height": 11, "fill": L["plume"], "x": px + SWAY.replace("2.4", "3.4"), "y": py})
    if rl == 'guard':
        # silks: saffron one side, white the other; a flip wave rolls through the guard (bars 7-8, 15-16)
        k = (i % 12) * 0.22 + (i // 12) * 0.5
        flip = ("=(gt > 26.9 || sin(gt * 4.2 - %.2f) > 0) ? #F3B231 : #F4F1E8" % k)
        gx_, gy_ = at(20, -2)
        out.append({"shape": "rect", "width": 44, "height": 26, "radius": 2, "fill": flip,
                    "x": gx_, "y": gy_, "rotation": "=gt > 26.9 ? 0 : 8 * sin(gt * 4.2 - %.2f)" % k, "opacity": "=gt > 26.6 ? max(0, 0.97 - (gt - 26.6) * 3) : 0.97"})
    if rl == 'brass':
        # sun (or stadium light) catching the bell: a glint wave
        k = (i % 12) * 0.35 + (i // 12) * 0.28
        gx_, gy_ = at(-2, 13)
        out.append({"shape": "circle", "width": 9, "height": 9, "fill": "#FFFFFF", "x": gx_, "y": gy_,
                    "blend": "add",
                    "opacity": "=pow(max(0, sin(gt * 3.4 - %.2f)), 14)" % k})
    return out

def dots_layers():
    # the page: painted yellow dots where set 1 lands, sprayed on the snare in bars 1-2
    pts = block(-6.0)
    snare = "x.xxX.x.x.xxX.x.x.x.X.xxx.x.X.x."
    hits = [k * BEAT / 4 for k, ch in enumerate(snare) if ch in 'xX']
    focus = (0.875, 3.625)
    order = sorted(range(N), key=lambda j: math.dist(pts[j], focus))
    out = []
    for rank, j in enumerate(order):
        t0 = hits[min(rank, len(hits) - 1)] if rank < 18 else max(0.0, hits[rank % len(hits)] - 0.6)
        x, y = W(pts[j])
        out.append({"shape": "circle", "width": 15, "height": 15, "fill": "#F5C41D", "opacity": 0.88,
                    "x": x, "y": y, "start": round(t0, 3),
                    "animate": [{"from": {"scale": 2.2, "opacity": 0}, "to": {"scale": 1, "opacity": 0.88},
                                 "duration": 0.09, "ease": "outCubic"}]})
    return out

def paint_text(text, xy, size, start=None, wipe=None, op=0.9, font="Alfa Slab One", spacing="0.02em", end=None):
    x, y = W(xy)
    l = {"text": text, "font": font, "size": size, "color": "#F3F1E8", "opacity": op,
         "x": x, "y": y, "letterSpacing": spacing}
    if start is not None:
        l["start"] = start
    if end is not None:
        l["end"] = end
    if wipe:
        l["enter"] = {"preset": "wipe", "duration": wipe}
    return l

def field(night):
    L = NIGHT if night else DAY
    layers = []
    if not night:
        layers += dots_layers()
    layers.append(paint_text("Your agent writes one page.", (0, 11.6), 88, start=bar(3), end=bar(5)))
    layers.append(paint_text("ANIKIT", (0, -2.5), 531.1, start=bar(14), op=0.82, spacing="0em"))
    layers.append(paint_text("VIDEO", (0, 8.6), 150, spacing="0.5em", start=bar(14)))
    layers.append(paint_text("anikit-video render spec.json", (0, 13.4), 74, start=28.15, wipe=0.5, font="Alfa Slab One"))
    for i in range(N):
        layers += marcher_layers(i, L, night)
    return layers

# Paint text uses video time for start; groups use scene time, so text starts are re-based per scene.
def rebase(layers, t0, dur=99):
    out = []
    for l in layers:
        if 'start' in l or 'end' in l:
            l = dict(l)
            if 'start' in l: l['start'] = round(l['start'] - t0, 4)
            if 'end' in l: l['end'] = round(l['end'] - t0, 4)
            if l.get('end', 1) <= 0 or l.get('start', 0) >= dur: continue
        out.append(l)
    return out

# ---------------------------------------------------------------- cameras
def ease_expr(a, b, d, kind='inOutSine', delay=0.0):
    if a == b:
        return a
    s = "clamp((t - %g) / %g, 0, 1)" % (delay, d)
    if kind == 'inOutSine':
        e = "(0.5 - 0.5 * cos(3.14159 * %s))" % s
    elif kind == 'outCubic':
        e = "(1 - pow(1 - %s, 3))" % s
    elif kind == 'inOutCubic':
        e = "(%s < 0.5 ? 4 * pow(%s, 3) : 1 - pow(-2 * %s + 2, 3) / 2)" % (s, s, s)
    else:
        e = s
    return "(%g + (%g) * %s)" % (a, b - a, e)

def num(v):
    return v if isinstance(v, str) else "%g" % v

def shot(dur, cx, cy, z, r, night, extra=None, seam=None):
    night = night or os.environ.get("STYLE") == "night"
    cx, cy, z, r = map(num, (cx, cy, z, r))
    a = "((%s) * 0.0174533)" % r
    gx = "=960 - (%s) * (cos(%s) * (%s) - sin(%s) * (%s))" % (z, a, cx, a, cy)
    gy = "=540 - (%s) * (sin(%s) * (%s) + cos(%s) * (%s))" % (z, a, cx, a, cy)
    uni = {"camX": "=" + cx, "camY": "=" + cy, "zoom": "=" + z, "rot": "=-(%s)" % r,
           "night": 1.0 if night else 0.0, "sun": 0.0 if os.environ.get("STYLE") == "overcast" else 1.0}
    if seam is not None:
        uni["seam"] = seam
    turf = {"id": "turf", "type": "shader", "src": "src/turf.glsl", "supersample": False, "uniforms": uni}
    grp = {"use": "night" if night else "day", "x": gx, "y": gy, "scale": "=" + z, "rotation": "=" + r}
    sc = {"duration": round(dur, 4), "background": "#1B3412", "layers": [turf, grp]}
    if extra:
        sc["layers"] += extra
    return sc

def yd(v):
    return v * Y

scenes = []
# S1 · bars 1-2 · macro: dots sprayed on the snare, shadows stride in, the front rank lands on bar 3
fx, fy = yd(0.875), yd(3.625)
scenes.append(shot(bar(3), ease_expr(fx - 140, fx + 10, bar(3)), ease_expr(fy - 120, fy - 30, bar(3)),
                   ease_expr(3.4, 3.0, bar(3)), ease_expr(16, 10, bar(3)), False))
# S2 · bars 3-4 · pull back: the whole block marches its eight steps; the page's words in the turf
scenes.append(shot(BAR * 2, ease_expr(fx, 0, BAR * 2, 'outCubic'), ease_expr(fy, yd(1.6), BAR * 2, 'outCubic'),
                   ease_expr(2.8, 1.0, BAR * 2, 'outCubic'), ease_expr(10, 0, BAR * 2, 'outCubic'), False))
# S3 · bars 5-6 · rotated: the block peels into the outline of a gull
scenes.append(shot(BAR * 2, ease_expr(yd(-3), yd(-12), BAR * 2), ease_expr(yd(-2), 0, BAR * 2),
                   ease_expr(1.3, 1.0, BAR * 2), ease_expr(-25, -90, BAR * 2, 'inOutCubic'), False))
# S4 · bars 7-8 · straight down, tracking: the gull flies, its wings beating on the drum
scenes.append(shot(BAR * 2, ease_expr(yd(-12), yd(-1.5), BAR * 2, 'linear'), 0, 1.0, -90, False))
# S5 · bars 9-10 · the same flight, another night: the seam sweeps afternoon into floodlight
seam_x = ease_expr(1800, -500, BAR * 2, 'inOutCubic')
scenes.append(shot(BAR * 2, ease_expr(yd(-1.5), yd(9), BAR * 2, 'linear'), 0, 1.0, -88, False, seam="=" + seam_x))
# S6 · bars 11-12 · night, close, tracking along the company front as it forms
scenes.append(shot(BAR * 2, ease_expr(yd(-14), yd(2), BAR * 2), ease_expr(yd(2.0), yd(2.6), BAR * 2),
                   2.3, -7, True))
# S7 · bar 13 · the hard stop: horns up, silence, one player close
scenes.append(shot(BAR, yd(6), yd(3.2), ease_expr(2.7, 2.85, BAR), 9, True))
# S8 · bars 14-16 · payoff: they peel into the name as the camera rises
scenes.append(shot(BAR * 3, 0, ease_expr(yd(1.0), yd(1.4), BAR * 3), "(" + ease_expr(2.1, 0.80, 2.4, 'inOutCubic') + " - 0.06 * clamp((t - 2.4) / 3.2, 0, 1))",
                   ease_expr(-5, 0, 2.4, 'inOutCubic'), True))

# S5 needs the night show too, matted to the right of the seam
s5 = scenes[4]
tilt = 0.18
s5["layers"].insert(1, {"id": "nightside", "shape": "path",
                        "d": "M0 0 L3600 0 L3600 1080 L%g 1080 Z" % (1080 * tilt), "fill": "#ffffff",
                        "anchor": "topLeft", "x": "=" + seam_x + " - %g" % (540 * tilt), "y": 0})
night_grp = dict(s5["layers"][2]); night_grp["use"] = "night"; night_grp["matte"] = "nightside"
s5["layers"].append(night_grp)

# per-scene time base for start times inside the field group
t = 0.0
for sc in scenes:
    for l in sc["layers"]:
        if l.get("use") in ("day", "night"):
            l["with"] = {}
    sc["_t0"] = t
    t += sc["duration"]

total = t
assert abs(total - 30.0) < 1e-3, total

spec = {
    "size": "16:9", "fps": 30,
    "intent": {"pace": "fast"},
    "music": json.load(open(os.path.join(HERE, 'music.json'))),
    "post": {"grain": 0.05, "vignette": 0.34},
    "defs": {},
    "scenes": [],
}
# defs can't carry per-scene start offsets, so each scene gets its own rebased copy of the few
# timed layers (dots, CTA) and shares the marchers through defs.
spec["defs"]["day"] = spec["defs"]["night"] = {}
for sc in scenes:
    t0 = sc.pop("_t0")
    for l in sc["layers"]:
        if l.get("use") in ("day", "night"):
            l.pop("with", None)
            l["start"] = 0
            l["_t0"] = t0
    spec["scenes"].append(sc)

# expand uses with rebased starts (only dots/CTA carry starts)
for sc in spec["scenes"]:
    new = []
    for l in sc["layers"]:
        if l.get("use") in ("day", "night"):
            t0 = l.pop("_t0"); kind = l.pop("use"); l.pop("start", None)
            WIN[0], WIN[1] = t0, t0 + sc["duration"]
            g = dict(l); g["layers"] = rebase(field(kind == "night"), t0, sc["duration"])
            new.append(g)
        else:
            new.append(l)
    sc["layers"] = new
del spec["defs"]

json.dump(spec, open(OUT, 'w'), separators=(',', ':'))
print("wrote", OUT, os.path.getsize(OUT) // 1024, "KB", "scenes", len(spec["scenes"]))
