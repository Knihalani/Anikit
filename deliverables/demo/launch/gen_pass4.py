#!/usr/bin/env python3
"""Railyard: "Arrivals". A ray-traced split-flap departure board for money.

Every cell is a real split-flap mechanism in scene3d: a static top half, a static bottom half and a
falling flap hinged at the middle. A cell can't skip letters; it clatters through its alphabet in order,
one flap every DT seconds, and each cell stops on its own time. The board's state is simulated here and
written as per-object `=expressions` of t (texture frame + flap angle), so the whole board is one scene.
"""
import json, math, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
FPS = 30
DUR = 15.0
DT = 0.07                       # a clattering flap: over two frames, so the hinge reads
DT1 = 0.16                     # Railyard's flap: the whole cell turns once, slowly, in one go                      # one flap (s)
COLS, ROWS = 26, 7
CW, CH = 0.150, 0.230           # cell pitch (m)
FW, FH = 0.136, 0.104           # flap half size (w, half-height)
ALPHA = " ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789$.,:/+>-"
N = len(ALPHA)
INKS = {"w": "#EEE7D7", "a": "#F4A63A", "g": "#3FDC86"}
INK_ORDER = ["w", "a", "g"]
QUICK = "--quick" in sys.argv

def cx(c): return (c - (COLS - 1) / 2) * CW
def cy(r): return ((ROWS - 1) / 2 - r) * CH + 0.05

# ------------------------------------------------------------------ board script
# (time, row, text, ink, stagger per column s, start column)
def pad(s): return (s + " " * COLS)[:COLS]
EVENTS = []
def put(t, r, text, ink="w", stagger=0.012, col0=0, only=None):
    EVENTS.append((t, r, text, ink, stagger, col0, only))

# waiting money already on the board; the statuses are still landing as we open
def row(tm, name, amt): return pad("%s %-5s%7s " % (tm, name, amt))
put(-0.6, 1, row("09:12", "WIRE", "$8,200"), "w", 0.0)
put(-0.6, 2, row("09:20", "ACH", "$2,315"), "w", 0.0)
put(-0.6, 3, row("09:33", "PAY", "$41,000"), "w", 0.0)
put(-0.6, 4, row("09:38", "INV", "$960"), "w", 0.0)
put(-0.6, 0, pad("TRANSFERS   TIME 09:41:06"), "w", 0.0)
def status(t, r, word, ink, stg=0.03, mode=None):
    put(t, r, " " * 19 + (word + " " * 7)[:7], ink, stg, 19, mode)
status(0.05, 1, "DELAYED", "a", 0.02)
status(0.55, 2, "QUEUED", "a")
status(0.9, 3, "HELD", "a")
status(1.25, 4, "QUEUED", "a")
# the new transfer: one API call
put(3.3, 5, pad("09:41 POST /V1/TRANSFERS"), "w", 0.0, 0, "instant")
put(5.0, 5, row("09:41", "API", "$12,400"), "w", 0.0, 0, "instant")
status(5.9, 5, "CREATED", "w", 0.0, "instant")
put(5.9, 6, pad("WEBHOOK > TRANSFER.CREATED"), "w", 0.0, 0, "instant")
status(7.0, 5, "PENDING", "a", 0.0, "instant")
put(7.0, 6, pad("WEBHOOK > TRANSFER.PENDING"), "a", 0.0, 0, "instant")
status(8.1, 5, "SETTLED", "g", 0.0, "instant")
put(8.1, 6, pad("WEBHOOK > TRANSFER.SETTLED"), "g", 0.0, 0, "instant")
put(8.35, 0, pad("TRANSFERS   TIME 09:41:07"), "w", 0.0)
# escalation: the waiting rows give up waiting, one after another, green
for k, r in enumerate([4, 3, 2, 1]):
    status(8.75 + 0.08 * k, r, "SETTLED", "g", 0.02, "instant")
put(8.75, 0, pad("TRANSFERS   DELAYED: 0"), "g", 0.0, 0, "instant")
# payoff: the whole board clatters into the name and the line
T_PAY = 9.9
put(T_PAY, 0, pad(""), "w", 0.0, 0, "instant")
put(T_PAY, 1, pad(""), "w", 0.0, 0, "instant")
put(T_PAY, 2, pad("         RAILYARD"), "g", 0.0, 0, "instant")
put(T_PAY, 3, pad(""), "w", 0.0, 0, "instant")
put(T_PAY, 4, pad("       ONE API CALL."), "w", 0.0, 0, "instant")
put(T_PAY, 5, pad("    SETTLED INSTANTLY."), "w", 0.0, 0, "instant")
put(T_PAY, 6, pad(""), "w", 0.0, 0, "instant")
# button
put(13.9, 0, pad("            TIME 09:41:07"), "w", 0.0, 0, "instant")
T_BTN = 12.7
put(T_BTN, 4, pad("       DELAYS: NONE"), "w", 0.0, 0, "instant")
put(T_BTN, 5, pad(""), "w", 0.0, 0, "instant")
put(T_BTN, 6, pad(""), "w", 0.0, 0, "instant")

# ------------------------------------------------------------------ simulate
# per cell: list of runs (T, start index, steps, ink before, ink after)
cells = {(r, c): {"cur": 0, "ink": "w", "free": -1e9, "runs": []} for r in range(ROWS) for c in range(COLS)}
for (t, r, text, ink, stg, col0, only) in sorted(EVENTS, key=lambda e: e[0]):
    for c in range(COLS):
        ch = text[c] if c < len(text) else " "
        if col0 and c < col0:
            continue
        cell = cells[(r, c)]
        tgt = ALPHA.index(ch) if ch in ALPHA else 0
        if tgt == cell["cur"] and ink == cell["ink"]:
            continue
        if "init" not in cell and t >= 0:
            cell["init"] = (cell["cur"], cell["ink"])     # what the cell shows before its first run
        T = max(t + stg * (c - col0), cell["free"])
        dt = DT1 if only == "instant" else DT
        steps = (tgt - cell["cur"]) % N
        old = (cell["cur"], cell["ink"])
        if steps == 0:
            k_ = 1 if only == "instant" else 4
            cell["cur"] = (tgt - k_) % N   # a colour change: a short turn of the drum
            steps = k_
        MAXS = 1 if only == "instant" else 5 + ((r * 7 + c * 3) % 4)  # Railyard lands in one flap; the old rails clatter
        if steps > MAXS:
            cell["cur"] = (tgt - MAXS) % N
            steps = MAXS
        if T < 0:                          # already on the board at t=0
            cell["cur"], cell["ink"] = tgt, ink
            continue
        cell["runs"].append((T, cell["cur"], steps, old, ink, dt))
        cell["cur"] = tgt
        cell["ink"] = ink
        cell["free"] = T + steps * dt
INIT = {}
# initial state = state before first run
for k, cell in cells.items():
    if cell["runs"]:
        INIT[k] = cell["init"]
    else:
        INIT[k] = (cell["cur"], cell["ink"])

def fr(ink, idx_expr):
    return "%d + mod(%s, %d)" % (INK_ORDER.index(ink) * N, idx_expr, N)

def cell_exprs(k):
    """Expressions (top, bottom, flap front, flap back, flap angle) for a cell."""
    runs = cells[k]["runs"]
    c0, ink0 = INIT[k]
    init = str(INK_ORDER.index(ink0) * N + c0)
    if not runs:
        return init, init, init, init, "0"
    S_ = lambda T, n, d: "clamp(floor((t-%.4f)/%.4f), 0, %d)" % (T, d, n)
    F_ = lambda T, n, d: "(t>=%.4f && t<%.4f)" % (T, T + n * d)
    def chain(fn, e):
        for (T, c, n, inkA, inkB, d) in runs:
            e = "(t>=%.4f ? %s : %s)" % (T, fn(T, c, n, inkB, d), e)
        return e
    top = chain(lambda T, c, n, ink, d: fr(ink, "%d + %s + (%s ? 1 : 0)" % (c, S_(T, n, d), F_(T, n, d))), init)
    # until the first flap lands, the bottom half and the falling flap still show the old letter
    OLD = {T: o for (T, c, n, o, inkB, d) in runs}
    def oldfr(T): o = OLD[T]; return str(INK_ORDER.index(o[1]) * N + o[0])
    bot = chain(lambda T, c, n, ink, d: "(t<%.4f ? %s : %s)" % (T + d, oldfr(T), fr(ink, "%d + %s" % (c, S_(T, n, d)))), init)
    ff = chain(lambda T, c, n, ink, d: "(t<%.4f ? %s : %s)" % (T + d, oldfr(T), fr(ink, "%d + %s" % (c, S_(T, n, d)))), init)
    fb = chain(lambda T, c, n, ink, d: fr(ink, "%d + %s + 1" % (c, S_(T, n, d))), init)
    def angf(T, c, n, ink, d):
        u = "fract((t-%.4f)/%.4f)" % (T, d)
        # falls under gravity, hits the stop at 180 and bounces back a little
        return "(%s ? 180*min(1, 1.3*%s*%s) - 14*max(0, 1.3*%s*%s-1)*(1-%s) : 0)" % (F_(T, n, d), u, u, u, u, u)
    a = chain(angf, "0")
    return top, bot, ff, fb, a

# ------------------------------------------------------------------ glyph compositions
def glyph_comp(kind):
    # kind: top (upper half), bot (lower half), bback (lower half, flipped for the flap's back face)
    layers = []
    for ii, ink in enumerate(INK_ORDER):
        for k, ch in enumerate(ALPHA):
            f = ii * N + k
            L = {"text": ch if ch != " " else " ", "font": "Barlow Condensed", "weight": 600, "size": 168,
                 "color": INKS[ink], "x": 68, "anchor": "center",
                 "start": round((f - 0.3) / FPS, 5), "end": round((f + 0.7) / FPS, 5)}
            L["y"] = 104 if kind == "top" else 0
            if kind == "bback":
                L["scaleY"] = -1
                L["y"] = 104
            layers.append(L)
    bg = "#1d1f1e" if kind == "top" else "#191b1a"
    # a faint printed hinge line at the split
    hinge_y = 102 if kind == "top" else 2
    return {"size": [136, 104], "duration": round(3 * N / FPS, 4), "background": bg, "layers": layers + [
        {"shape": "rect", "width": 136, "height": 3, "x": 68, "y": hinge_y, "fill": "#0b0c0b"}]}

# ------------------------------------------------------------------ scene
objects = []
# housing: deep enamel box, the board inset
objects.append({"type": "box", "size": [COLS * CW + 0.5, ROWS * CH + 0.62, 0.3], "pos": [0, 0.07, -0.19], "material": "housing"})
# column labels printed on the housing
objects.append({"type": "pane", "size": [COLS * CW, 0.09, 0.002], "pos": [0, cy(0) + CH / 2 + 0.07, -0.035], "material": "matte",
                "texture": {"composition": "labels", "frame": 0, "face": "front"}})
# per-row status lamps
for r in range(1, 6):
    pass
cellset = [(r, c) for r in range(ROWS) for c in range(COLS)]
if QUICK:
    cellset = [(r, c) for r in range(1, 3) for c in range(14, 26)]
for (r, c) in cellset:
    top, bot, ff, fb, ang = cell_exprs((r, c))
    x, y = cx(c), cy(r)
    jit = ((r * 31 + c * 17) % 7 - 3) * 0.0015      # each cell a hair different
    objects.append({"type": "slab", "size": [FW, FH, 0.007], "pos": [round(x, 4), round(y + FH / 2 + 0.002, 4), -0.006],
                    "material": "flap", "texture": {"composition": "top", "frame": "=" + top, "face": "front"}})
    objects.append({"type": "slab", "size": [FW, FH, 0.007], "pos": [round(x, 4), round(y - FH / 2 - 0.002, 4), -0.006],
                    "material": "flap", "texture": {"composition": "bot", "frame": "=" + bot, "face": "front"}})
    a = "(%s)" % ang
    objects.append({"type": "slab", "size": [FW, FH, 0.007],
                    "pos": "=[%.4f, %.4f + %.4f*cos(%s*0.017453), %.4f*sin(%s*0.017453) + 0.0015]" % (x, y, FH / 2 + 0.002, a, FH / 2 + 0.002, a),
                    "rot": "=[-%s + %.3f, 0, 0]" % (a, jit * 40),
                    "material": "flap",
                    "texture": [{"composition": "top", "frame": "=" + ff, "face": "front"},
                                {"composition": "bback", "frame": "=" + fb, "face": "back"}]})

# hinge axles: one steel rod per row, the flaps hang on it
pass
# status lamps: amber while waiting, green once settled
def lamp(r, ton_amber, tgreen, toff):
    return {"type": "sphere", "size": 0.018, "pos": [round(cx(COLS - 1) + CW * 0.75, 4), round(cy(r), 4), 0.0], "material": "emissive",
            "color": "=t>=%.3f ? (t>=%.3f ? #1a1a1a : #3FDC86) : (t>=%.3f ? #F4A63A : #1a1a1a)" % (tgreen, toff, ton_amber)}
lamps = []

# camera choreography (pos, target, focal, fstop, focus)
B = cy(1)
def V(x, y, z): return [round(x, 3), round(y, 3), round(z, 3)]
def ease(u): u = max(0.0, min(1.0, u)); return u * u * (3 - 2 * u)
def ease_out(u): u = max(0.0, min(1.0, u)); return 1 - (1 - u) ** 3
# (t0, t1, from (pos, target, focal, fstop), to, easing): each shot a deliberate move; cuts land on flips
SHOTS = [
    # 1 hook: raking macro along the status column as DELAYED lands, low and from the side, so flaps read as objects
    (0.0, 1.9, ([cx(25) + 0.35, cy(1) - 0.10, 0.55], [cx(20), cy(1), 0.0], 60, 2.2),
               ([cx(25) + 0.22, cy(1) - 0.06, 0.62], [cx(19.5), cy(1.1), 0.0], 60, 2.2), ease),
    # 2 the board of waiting money, frontal, slow push
    (1.9, 4.3, ([0.0, 0.18, 6.1], [0.0, 0.18, 0.0], 50, 8.0), ([0.0, 0.15, 5.6], [0.0, 0.15, 0.0], 50, 8.0), ease),
    # 3 three-quarter on the new row as the API call clatters in
    (4.3, 6.3, ([cx(2) - 1.2, cy(5) + 0.35, 2.2], [cx(9), cy(5.2), 0.0], 50, 4.0), ([cx(2) - 0.9, cy(5) + 0.3, 2.0], [cx(10), cy(5.2), 0.0], 50, 4.0), ease),
    # 4 the hero: tight on its status and the webhook row, CREATED > PENDING > SETTLED
    (6.3, 8.9, ([cx(23) + 0.9, cy(5.5) - 0.05, 2.0], [cx(22.2), cy(5.5), 0.0], 60, 3.2), ([cx(23) + 0.6, cy(5.5) - 0.02, 1.8], [cx(22.2), cy(5.5), 0.0], 60, 3.2), ease_out),
    # 5 the peak: the wave runs down the status column; the camera is pushed back by it
    (8.9, 9.9, ([cx(25) + 0.7, cy(2.6), 2.2], [cx(19), cy(2.8), 0.0], 40, 5.6), ([cx(25) + 1.0, cy(2.6) + 0.05, 2.7], [cx(18.5), cy(2.8), 0.0], 40, 5.6), ease_out),
    # 6 payoff: frontal, the whole board in frame, the name lands and holds; a slow push
    (9.9, 12.7, ([0.0, 0.12, 6.0], [0.0, 0.12, 0.0], 50, 8.0), ([0.0, 0.12, 5.5], [0.0, 0.12, 0.0], 50, 8.0), ease),
    # 7 button: DELAYS: NONE, the rhyme with frame 1, fully in frame
    (12.7, 15.0, ([cx(12.5), cy(3.4), 3.6], [cx(12.5), cy(3.4), 0.0], 50, 5.6), ([cx(12.5), cy(3.4), 3.25], [cx(12.5), cy(3.4), 0.0], 50, 5.6), ease_out),
]
CUTS = [sh[0] for sh in SHOTS[1:]]
cam_keys = []
for (t0, t1, A, Bk, fn) in SHOTS:
    nfr = int(round((t1 - t0) * FPS))
    for j in range(nfr + 1):
        tt = t0 + j / FPS
        if j == nfr:
            tt = t1 - 0.0005
        u = fn((tt - t0) / (t1 - t0))
        pos = [round(a_ + (b_ - a_) * u, 4) for a_, b_ in zip(A[0], Bk[0])]
        tgt = [round(a_ + (b_ - a_) * u, 4) for a_, b_ in zip(A[1], Bk[1])]
        dist = math.dist(pos, tgt)
        cam_keys.append({"t": round(tt, 4), "pos": pos, "target": tgt, "focal": A[2], "fstop": A[3], "focus": round(dist, 3), "shutter": 0.5})

scene3d = {
    "env": {"preset": "studio", "color": "#0b0d0c"},
    "floor": False,
    "fog": {"density": 0.009, "color": "#120f0b", "shafts": True},
    "quality": "draft" if QUICK else "standard",
    "exposure": 1.25, "bloom": 0.12, "vignette": 0.35, "tonemap": "aces",
    "materials": {
        "flap": {"extends": "matte", "base": "#1c1e1d", "roughness": 0.22, "specular": 0.8, "clearcoat": 0.3, "imperfections": 0.55},
        "housing": {"extends": "matte", "base": "#0e1411", "roughness": 0.45, "specular": 0.35, "clearcoat": 0.2, "imperfections": 0.6},
    },
    "lights": [
        # the dispatcher's practical: a warm hooded lamp above the board, falloff top to bottom
        {"pos": [-0.8, 1.55, 0.9], "target": [0.6, 0.0, 0], "angle": 80, "penumbra": 0.9, "intensity": 34, "color": "#FFD39A"},
        # a grazing rim from the upper right: catches every flap edge
        {"pos": [3.4, 1.4, 0.35], "target": [0, 0, 0], "angle": 50, "penumbra": 0.7, "intensity": 14, "color": "#E8EEF2", "shadow": False},
        # a cold spill from a window on the left
        {"pos": [-4.5, 0.5, 3.5], "target": [0, 0, 0], "angle": 40, "penumbra": 1.0, "intensity": 5, "color": "#7FA6C8", "shadow": False},
        # the settle: a green wash rises from the status column when the money arrives, then becomes the brand light
        {"pos": [cx(22), -1.2, 1.6], "target": [cx(20), cy(4), 0], "angle": 60, "penumbra": 1.0, "color": "#3FDC86", "shadow": False,
         "intensity": "=10*smoothstep(8.1, 9.2, t)*(1 - 0.5*smoothstep(10.5, 11.5, t))"},
    ],
    "camera": {"interp": "linear", "keys": cam_keys, "shake": "breath"},
    "objects": objects,
}

labels = {"size": [int(COLS * 136), 90], "duration": 1, "background": "#0d110f", "layers": [
    {"text": t_, "font": "Barlow Condensed", "weight": 500, "size": 46, "letterSpacing": "0.2em", "color": "#9a978d",
     "x": int(x_ * 136), "y": 45, "anchor": "left"} for t_, x_ in
    [("SENT", 0.15), ("RAIL", 6.15), ("AMOUNT", 11.15), ("STATUS", 19.15)]]}

spec = {
    "size": [1920, 1080], "fps": FPS, "transition": "cut", "background": "#0b0d0c",
    "intent": {"pace": "steady", "feeling": "deadpan, mechanical, then release"},
    "compositions": {"top": glyph_comp("top"), "bot": glyph_comp("bot"), "bback": glyph_comp("bback"), "labels": labels},
    "scenes": [{"duration": DUR, "layers": [{"component": "scene3d", "props": scene3d}]}],
}

# flap clack times for the sound
clacks = []
for k, cell in cells.items():
    if QUICK and k not in cellset:
        continue
    for (T, c, n, a, b, d) in cell["runs"]:
        for s_ in range(n):
            clacks.append(round(T + s_ * d + d * 0.78, 3))
clacks.sort()
json.dump(clacks, open(os.path.join(HERE, "clacks.json"), "w"))
# ---- sound: the clatter is the rhythm section. Flap clacks binned per frame; louder where more cells fall.
import collections
bins = collections.Counter(round(c * 60) for c in clacks if 0 <= c < DUR)
cues = []
for b, n in sorted(bins.items()):
    cues.append({"sfx": "tick", "at": round(b / 60, 4), "gain": round(-27 + 5 * math.log2(1 + n), 1), "pitch": (b * 7) % 5 - 2,
                 "x": round(((b * 13) % 11 - 5) / 10, 2), "duck": 0})
for tt, pitch in ((5.9, 0), (7.0, 2), (8.1, 5)):
    cues.append({"sfx": "chime", "at": tt + 0.14, "gain": -6, "pitch": pitch, "x": 0.4})
cues.append({"sfx": "riser", "at": 8.75, "gain": -12})
cues.append({"sfx": "impact", "at": 8.89, "gain": -8})
cues.append({"sfx": "thump+wood", "at": T_PAY + 0.14, "gain": -4})
cues.append({"sfx": "wood", "at": T_BTN + 0.14, "gain": -7})
cues.append({"sfx": "ding", "at": T_BTN + 0.3, "gain": -10, "pitch": 7})
spec["sound"] = {"design": "off", "palette": "organic", "ambience": {"type": "room", "level": -30}, "space": "room", "cues": cues}
spec["music"] = {"bpm": 120, "key": "Dm", "length": "15s", "seed": 4, "end": "ring", "master": {"reverb": "room"},
  "tracks": [
    {"inst": "piano", "notes": "o3 [D3 A3 F4]/1 | r/1 | [Bb2 F3 D4]/1 | [A2 E3 C#4]/2 r/2 | [D3 F#3 A3 D4]/1 | r/1 | [G2 D3 B3]/1 | [D3 A3 F#4 D5]/1", "gain": -10, "reverb": 0.35},
    {"inst": "cello", "notes": "o2 D2/1 | D2/1 | Bb1/1 | A1/1 | D2/1 | D2/1 | G1/1 | D2/1", "gain": -14},
    {"inst": "upright-bass", "notes": "o2 r/1 | r/1 | Bb1/2 r/2 | A1/2 r/2 | D2/1 | D2/1 | G1/1 | D2/1", "gain": -8},
    {"inst": "strings", "notes": "o4 r/1 | r/1 | r/1 | r/2 [A3 E4]/2 | [D4 F#4 A4]/1 | _ | [D4 G4 B4]/1 | [D4 F#4 A4 D5]/1", "gain": -12, "reverb": 0.4},
    {"inst": "pad-warm", "notes": "o3 r/1 | r/1 | [D3 A3]/1 | [D3 A3 E4]/1 | [D3 F#3 A3]/1 | _ | r/1 | r/1", "gain": -16,
     "automate": {"volume": [[8, -20], [16, 0], [18, 2]]}},
    {"inst": "glockenspiel", "notes": "o6 r/1 | r/1 | r/1 | r/1 | r/2 A6/8 F#6/8 D6/4 | r/1 | r/1 | r/2 F#6/8 A6/8 D7/4", "gain": -14}
  ]}
out = os.path.join(HERE, "spec.json")
json.dump(spec, open(out, "w"))
print("objects", len(objects), "clacks", len(clacks), "spec MB", round(os.path.getsize(out) / 1e6, 1))
