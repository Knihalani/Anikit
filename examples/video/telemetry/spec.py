"""Builds spec.json for the "Telemetry" film from the race simulation.

    python3 examples/video/telemetry/spec.py     # runs race.py's simulation, writes race.json + spec.json

Every position, speed, gap and lap time on screen is read from the simulation
in race.py; this file only decides layout, timing and look.
"""

import json
import math
import os

import race

HERE = os.path.dirname(os.path.abspath(__file__))
W, H = 1920, 1080
FPS = 60

# ---- design tokens -------------------------------------------------------------
BG = "#05070B"
PANEL = "#0B1018"
PANEL2 = "#121926"
LINE = "#FFFFFF1F"
TEXT = "#F2F5FA"
MUTED = "#8792A6"
RED = "#FF2E3F"  # series colour
PURPLE = "#A65CFF"  # overall fastest
GREEN = "#2FD877"  # personal best
YELLOW = "#FFD23F"
MONO = "$mono"
HEAD = "$heading"
M = 64  # outer margin

DATA, API = race.simulate()
with open(os.path.join(HERE, "race.json"), "w") as f:
    json.dump(DATA, f, separators=(",", ":"))

TEAM = {d["code"]: d["team"] for d in DATA["drivers"]}
NAME = {d["code"]: d["name"] for d in DATA["drivers"]}
COL = {c: DATA["teams"][TEAM[c]] for c in TEAM}
CODES = [d["code"] for d in DATA["drivers"]]
TRACK = DATA["track"]
NS = len(TRACK)
SPEED = DATA["speed_kmh"]
frac_at = API["frac_at"]
FIRST = DATA and race.FIRST_LAP
STARTS = API["starts"]


def r(x, k=2):
    return round(x, k)


# ---- layer helpers -------------------------------------------------------------
def text(s, x, y, size, color=TEXT, font=MONO, weight=500, anchor="left", **kw):
    al = {"left": "left", "right": "right"}.get(anchor.lower().replace("top", "").replace("bottom", "") or "center", "center")
    size = max(size, 18)
    d = {"text": s, "x": r(x), "y": r(y), "size": size, "color": color, "font": font, "weight": weight, "anchor": anchor, "align": al}
    d.update(kw)
    return d


def rect(x, y, w, h, fill, anchor="topLeft", **kw):
    d = {"shape": "rect", "x": r(x), "y": r(y), "width": r(w), "height": r(h), "fill": fill, "anchor": anchor}
    d.update(kw)
    return d


def path(d, color, width, **kw):
    """A stroked path in frame coordinates. Paths are normalised to their bounding
    box, so the layer is placed at the box's top-left corner."""
    import re
    nums = [float(v) for v in re.findall(r"-?\d+(?:\.\d+)?", d)]
    xs, ys = nums[0::2], nums[1::2]
    lay = {"shape": "path", "d": d, "x": r(min(xs)), "y": r(min(ys)), "anchor": "topLeft", "fill": "none",
           "stroke": {"color": color, "width": width, "cap": "round", "join": "round"}}
    lay.update(kw)
    return lay


def group(layers, **kw):
    d = {"type": "group", "layers": layers}
    d.update(kw)
    return d


def poly_d(pts, close=False):
    s = "M" + " L".join(f"{p[0]:.1f} {p[1]:.1f}" for p in pts)
    return s + (" Z" if close else "")


def kf(series, duration, at=0, ease="linear"):
    """series: {prop: [(t_local, value), ...]} with absolute local times; returns a keyframes tween."""
    out = {}
    for prop, pts in series.items():
        out[prop] = [[r(min(1.0, max(0.0, (t - at) / duration)), 4), r(v, 3)] for t, v in pts]
    return {"keyframes": out, "at": r(at, 3), "duration": r(duration, 3), "ease": ease}


def steps(changes, t_end, tween=0.25):
    """[(t, value)] step changes → keyframe points that tween over `tween` s at each change."""
    pts = [(0.0, changes[0][1])]
    for (t, v), (_, prev) in zip(changes[1:], changes[:-1]):
        pts.append((t, prev))
        pts.append((min(t_end, t + tween), v))
    pts.append((t_end, changes[-1][1]))
    return pts


def track_point(f, xf):
    f = f % 1.0
    x = f * NS
    i = int(x) % NS
    j = (i + 1) % NS
    u = x - int(x)
    a, b = TRACK[i], TRACK[j]
    return xf(a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u)


def speed_at(f):
    x = (f % 1.0) * NS
    i = int(x) % NS
    return SPEED[i] + (SPEED[(i + 1) % NS] - SPEED[i]) * (x - int(x))


def sample_idx(f):
    return int((f % 1.0) * NS) % NS


def map_xf(cx, cy, scale):
    xs = [p[0] for p in TRACK]
    ys = [p[1] for p in TRACK]
    mx, my = (min(xs) + max(xs)) / 2, (min(ys) + max(ys)) / 2
    return lambda x, y: (cx + (x - mx) * scale, cy + (y - my) * scale)


def track_d(xf, f0=0.0, f1=1.0, step=3):
    i0, i1 = int(f0 * NS), int(f1 * NS)
    pts = [xf(*TRACK[i % NS]) for i in range(i0, i1 + 1, step)]
    return poly_d(pts, close=(f0 == 0.0 and f1 == 1.0))


def fmt_lap(t):
    m = int(t // 60)
    return f"{m}:{t - 60 * m:06.3f}"


# ---- shared pieces -------------------------------------------------------------
def bug(lap_changes, t_end, enter=0.0):
    """Top-left series bug: red tab, event name, lap counter."""
    lap_pts = steps(lap_changes, t_end, tween=0.2)
    return group([
        rect(M, 52, 92, 44, RED, skewX=-12),
        text("MFS", M + 46, 74, 24, "#fff", HEAD, 900, "center", letterSpacing="0.04em"),
        rect(M + 104, 52, 300, 44, PANEL),
        text("KESSRA GP", M + 122, 74, 22, TEXT, HEAD, 800, "left", letterSpacing="0.06em"),
        rect(M + 404, 52, 168, 44, PANEL2),
        text("LAP", M + 420, 74, 18, MUTED, MONO, 600, "left"),
        text("{value:0}", M + 506, 74, 24, TEXT, MONO, 700, "right", value=lap_changes[0][1],
             animate=[kf({"value": lap_pts}, t_end)]),
        text("/58", M + 508, 74, 24, MUTED, MONO, 500, "left"),
    ], enter={"preset": "fadeRight", "delay": enter, "duration": 0.5, "ease": "outExpo"})


def lower_third(code, line2, start, end, x=M, y=H - 150):
    c = COL[code]
    first, last = NAME[code].split(" ", 1)
    return group([
        rect(x, y, 10, 92, c),
        rect(x + 10, y, 560, 92, "#0B1018EE"),
        text(first.upper(), x + 36, y + 30, 22, MUTED, HEAD, 600, "left", letterSpacing="0.08em"),
        text(last.upper(), x + 36, y + 64, 40, TEXT, HEAD, 900, "left", letterSpacing="0.02em"),
        rect(x + 400, y + 18, 140, 56, c + "26", radius=4),
        text(code, x + 470, y + 47, 30, c, MONO, 800, "center"),
        rect(x + 10, y + 92, 560, 40, c),
        text(line2, x + 36, y + 113, 20, "#05070B", MONO, 700, "left", letterSpacing="0.04em"),
    ], start=start, end=end,
        enter={"preset": "wipeIn", "direction": "right", "duration": 0.45, "ease": "outExpo"},
        exit={"preset": "wipeOut", "direction": "left", "duration": 0.35})


def backdrop(seed=1):
    return [
        {"component": "grid", "props": {"cell": 64, "color": "#FFFFFF08", "lineWidth": 1, "fade": 0.9}},
        {"shape": "rect", "width": W, "height": H, "fill": "radial-gradient(#00000000, #000000B0)", "x": W / 2, "y": H / 2},
    ]


# ---- scene 1: open ---------------------------------------------------------------
def scene_open():
    xf = map_xf(1340, 540, 0.98)
    d = track_d(xf)
    dur = 4.4
    return {
        "id": "open", "duration": dur, "background": BG,
        "camera": {"animate": [{"from": {"zoom": 1.06}, "duration": dur, "ease": "outCubic"}]},
        "layers": backdrop() + [
            path(d, "#FFFFFF10", 34, animate=[{"from": {"trimEnd": 0}, "duration": 2.4, "ease": "inOutCubic"}]),
            path(d, RED, 4, glow={"radius": 18, "intensity": 0.8, "color": RED},
                 animate=[{"from": {"trimEnd": 0}, "duration": 2.4, "ease": "inOutCubic"}]),
            rect(M, 380, 12, 0, RED, animate=[{"to": {"height": 300}, "at": 0.15, "duration": 0.6, "ease": "outExpo"}]),
            text("MERIDIAN FORMULA SERIES  ·  ROUND 19", M + 44, 400, 24, RED, MONO, 700, "left", letterSpacing="0.12em",
                 enter={"preset": "typewriter", "delay": 0.2, "cps": 60}),
            text("KESSRA", M + 40, 486, 150, TEXT, HEAD, 900, "left", letterSpacing="-0.01em", italic=True,
                 split="chars", enter={"preset": "fadeUp", "stagger": 0.035, "delay": 0.35, "duration": 0.5, "ease": "outExpo"}),
            text("GRAND PRIX", M + 44, 610, 96, TEXT, HEAD, 300, "left", letterSpacing="0.04em", italic=True,
                 split="chars", enter={"preset": "fadeUp", "stagger": 0.03, "delay": 0.55, "duration": 0.5, "ease": "outExpo"}),
            text("FINAL LAPS  —  TELEMETRY RECAP", M + 44, 694, 26, MUTED, MONO, 500, "left", letterSpacing="0.1em",
                 enter={"preset": "fadeIn", "delay": 1.1}),
            group([
                rect(M + 44, 760, 470, 64, PANEL),
                rect(M + 44, 760, 6, 64, RED),
                text("LAP", M + 72, 792, 22, MUTED, MONO, 600, "left"),
                text("{value:0}", M + 200, 792, 40, TEXT, MONO, 800, "right", value=54,
                     animate=[kf({"value": [(0, 50), (1.6, 54)]}, 1.6, at=0.0, ease="outCubic")]),
                text("/ 58", M + 210, 792, 40, MUTED, MONO, 500, "left"),
                text("5.112 KM", M + 494, 792, 20, MUTED, MONO, 500, "right"),
            ], enter={"preset": "fadeUp", "delay": 1.2}),
        ],
    }


# ---- timing tower ------------------------------------------------------------------
ROW = 54
MAP_IVE_PASS = 5.6  # map-scene time of the IVE/LIN pass
OT_PASS = 3.6  # overtake-scene time of the OKA/MAR pass
TW = 420
TYRE = {"MAR": "H", "OKA": "S", "LIN": "M", "IVE": "M", "NOV": "H", "FER": "M", "WEN": "H", "DUF": "M", "CAS": "H", "HOL": "M"}
TYRE_COL = {"S": "#FF3B47", "M": "#FFD23F", "H": "#E8ECF2"}


def tower(t_video, T_of, t_end, x=M, y=150, n_rows=10, header="LAP"):
    """Live timing tower. t_video: sample times; T_of: video time → race time."""
    samples = [(t, T_of(t)) for t in t_video]
    orders = [(t, API["order_at"](T)) for t, T in samples]
    rows = []
    # header
    rows.append(rect(x, y - 54, TW, 46, PANEL2))
    rows.append(text("POS", x + 14, y - 31, 16, MUTED, MONO, 700, "left", letterSpacing="0.1em"))
    rows.append(text("GAP TO LEADER", x + TW - 16, y - 31, 16, MUTED, MONO, 700, "right", letterSpacing="0.1em"))
    for i in range(n_rows):
        rows.append(rect(x, y + i * ROW, TW, ROW - 2, PANEL if i % 2 == 0 else "#0D131D"))
        rows.append(text(str(i + 1), x + 26, y + i * ROW + ROW / 2 - 1, 26, TEXT, MONO, 700, "center"))
    # per-driver rows that move
    for code in CODES:
        pos = [(t, o.index(code)) for t, o in orders]
        changes = [pos[0]]
        for t, p in pos[1:]:
            if p != changes[-1][1]:
                changes.append((t, p))
        if changes[0][1] >= n_rows:
            continue
        ys = [(t, y + p * ROW + ROW / 2 - 1) for t, p in steps(changes, t_end, tween=0.45)]
        # gap to leader at each timing line (sector boundaries)
        gaps = []
        for t, T in samples:
            pass
        gap_changes = gap_series(code, samples)
        lay = [
            rect(48, ROW / 2 - 1, 6, ROW - 14, COL[code], anchor="left"),
            text(code, 66, ROW / 2 - 1, 30, TEXT, HEAD, 800, "left", letterSpacing="0.04em"),
            {"shape": "circle", "width": 28, "height": 28, "x": 180, "y": r(ROW / 2 - 1), "fill": "#05070B",
             "stroke": {"color": TYRE_COL[TYRE[code]], "width": 3}},
            text(TYRE[code], 180, ROW / 2 - 1, 18, TYRE_COL[TYRE[code]], MONO, 800, "center"),
        ]
        if gap_changes:
            first_val = gap_changes[0][1]
            if first_val < 0.0005:
                lay.append(text("LEADER", TW - 16, ROW / 2 - 1, 22, MUTED, MONO, 700, "right"))
            else:
                lay.append(text("+{value:0.000}", TW - 16, ROW / 2 - 1, 26, TEXT, MONO, 500, "right", value=r(first_val, 3),
                                animate=[kf({"value": steps(gap_changes, t_end, tween=0.3)}, t_end)]))
        # gain/loss flash
        for (t0, p0), (t1, p1) in zip(changes[:-1], changes[1:]):
            up = p1 < p0
            c = GREEN if up else RED
            lay.append(rect(0, 0, TW, ROW - 2, c, opacity=0, start=r(t1),
                            animate=[{"keyframes": {"opacity": [0, 0.35, 0.35, 0]}, "duration": 1.6, "ease": "linear"}]))
            pts = [[0, -7], [8, 6], [-8, 6]] if up else [[0, 7], [8, -6], [-8, 6]]
            lay.append({"shape": "polygon", "points": pts, "x": 216, "y": r(ROW / 2 - 1), "width": 16, "height": 14,
                        "fill": c, "start": r(t1), "enter": "popIn", "end": r(t1 + 3.5), "exit": "fadeOut"})
        rows.append(group(lay, x=x, y=0, anchor="topLeft", width=TW, height=ROW,
                          animate=[kf({"y": [(t, v - ROW / 2 + 1) for t, v in ys]}, t_end, ease="outCubic")]))
    return rows


def gap_series(code, samples):
    """Gap to the leader, updated as the car crosses each sector line."""
    out = []
    prev_seg = None
    for t, T in samples:
        f = frac_at(code, T)
        seg = int(f) * 3 + sum(1 for b in race.SECTORS[1:3] if (f % 1.0) >= b)
        if seg != prev_seg:
            # leader at this point: whoever got here first
            lap = int(f)
            bfrac = lap + race.SECTORS[seg % 3]
            tc = API["crossing_time"](code, bfrac)
            first = min(API["crossing_time"](c, bfrac) for c in CODES)
            out.append((t, max(0.0, tc - first)))
            prev_seg = seg
    return out


# ---- scene 2: map ----------------------------------------------------------------
def scene_map():
    dur = 9.0
    xf = map_xf(1210, 590, 1.12)
    d = track_d(xf)
    PLAY0, SPEEDUP = 1.1, 12.0
    T_pass = DATA["overtakes"]["IVE_LIN"]["t"]
    T0 = T_pass - (MAP_IVE_PASS - PLAY0) * SPEEDUP

    def T_of(t):
        return T0 + max(0.0, t - PLAY0) * SPEEDUP

    ts = [i / 30 for i in range(int(dur * 30) + 1)]
    layers = backdrop()
    layers += [
        path(d, "#141B27", 30),
        path(d, "#1A2332", 30, animate=[{"from": {"trimEnd": 0}, "duration": 0.9, "ease": "outCubic"}]),
        path(d, "#DDE3EC", 7, animate=[{"from": {"trimEnd": 0}, "duration": 0.9, "ease": "outCubic"}]),
    ]
    # sector colouring on top (thin)
    sc = [RED, "#3D7BFF", YELLOW]
    for k in range(3):
        f0, f1 = race.SECTORS[k], race.SECTORS[k + 1]
        layers.append(path(track_d(xf, f0, f1, 2), sc[k], 3, opacity=0.9,
                           animate=[{"from": {"trimEnd": 0}, "at": 0.5 + 0.12 * k, "duration": 0.5, "ease": "outCubic"}]))
        px, py = track_point(f0, xf)
        layers.append(text(f"S{k + 1}", px, py - 34, 18, sc[k], MONO, 800, "center", enter={"preset": "popIn", "delay": 1.0 + 0.15 * k}))
    # start/finish
    sx, sy = track_point(0.0, xf)
    layers.append(rect(sx, sy, 6, 40, "#fff", anchor="center", enter={"preset": "fadeIn", "delay": 1.0}))
    # turn labels
    for name, f in (("T1", 0.215), ("T4", 0.32), ("T7", 0.567), ("T11", 0.77), ("T14", 0.915)):
        px, py = track_point(f, xf)
        i = sample_idx(f)
        a, b = TRACK[(i - 8) % NS], TRACK[(i + 8) % NS]
        nx, ny = -(b[1] - a[1]), (b[0] - a[0])
        ln = math.hypot(nx, ny) or 1
        layers.append(text(name, px + nx / ln * 38, py + ny / ln * 38, 16, MUTED, MONO, 700, "center",
                           enter={"preset": "fadeIn", "delay": 1.2}))
    # cars
    for idx, code in enumerate(reversed(CODES)):
        pts = [(t, track_point(frac_at(code, T_of(t)), xf)) for t in ts]
        top = CODES.index(code) < 4
        car = [
            {"shape": "circle", "width": 22, "height": 22, "fill": COL[code], "stroke": {"color": "#05070B", "width": 3},
             "glow": {"radius": 14, "intensity": 0.7, "color": COL[code]}},
        ]
        if top:
            left = code in ("OKA", "LIN")
            lx = -58 if left else 34
            car.append(rect(lx, 20, 64, 28, "#05070BDD", anchor="left", radius=3))
            car.append(rect(lx + (60 if left else 0), 20, 4, 28, COL[code], anchor="left"))
            car.append(text(code, lx + 34, 20, 18, TEXT, MONO, 800, "center"))
        layers.append(group(car, width=40, height=40, x=r(pts[0][1][0]), y=r(pts[0][1][1]),
                            enter={"preset": "popIn", "delay": 1.0 + 0.04 * idx},
                            animate=[kf({"x": [(t, p[0]) for t, p in pts], "y": [(t, p[1]) for t, p in pts]}, dur)]))
    # lap counter follows the leader
    lead_lap = [(0.0, 55)]
    for t in ts:
        L = int(frac_at("MAR", T_of(t))) + FIRST
        if L != lead_lap[-1][1]:
            lead_lap.append((t, L))
    layers += tower(ts, T_of, dur)
    layers.append(bug(lead_lap, dur))
    # header right
    layers.append(group([
        text("KESSRA HARBOUR CIRCUIT", W - M, 74, 24, TEXT, HEAD, 800, "right", letterSpacing="0.08em"),
        text("5.112 KM  ·  17 TURNS  ·  LIVE ×12", W - M, 106, 18, MUTED, MONO, 500, "right", letterSpacing="0.06em"),
    ], enter={"preset": "fadeLeft", "delay": 0.2}))
    # battle callout when IVE passes LIN
    tp = PLAY0 + (T_pass - T0) / SPEEDUP
    px, py = track_point(frac_at("IVE", T_pass), xf)
    layers.append(group([
        {"shape": "circle", "width": 90, "height": 90, "fill": "none", "x": r(px), "y": r(py),
         "stroke": {"color": GREEN, "width": 3}, "animate": [{"from": {"scale": 0.2, "opacity": 1}, "to": {"scale": 1.6, "opacity": 0}, "duration": 0.9, "repeat": 1}]},
        rect(px + 60, py - 110, 300, 64, PANEL, anchor="topLeft"),
        rect(px + 60, py - 110, 6, 64, GREEN, anchor="topLeft"),
        text("OVERTAKE · T7", px + 80, py - 92, 16, GREEN, MONO, 800, "left", letterSpacing="0.08em"),
        text("IVE", px + 80, py - 64, 26, TEXT, HEAD, 800, "left"),
        {"shape": "polygon", "points": [[0, -8], [9, 6], [-9, 6]], "x": r(px + 150), "y": r(py - 66), "width": 18, "height": 14, "fill": GREEN},
        text("P3", px + 172, py - 64, 26, TEXT, HEAD, 800, "left"),
    ], start=r(tp - 0.1), end=r(tp + 2.6), enter={"preset": "fadeIn", "duration": 0.2}, exit="fadeOut"))
    return {"id": "map", "duration": dur, "background": BG,
            "transition": {"type": "slide", "direction": "left", "duration": 0.5, "ease": "inOutQuart"},
            "layers": layers}


# ---- scene 3: onboard telemetry -----------------------------------------------------
def arc_pts(cx, cy, rad, a0, a1, n=96):
    return [(cx + rad * math.cos(math.radians(a0 + (a1 - a0) * k / n)), cy + rad * math.sin(math.radians(a0 + (a1 - a0) * k / n))) for k in range(n + 1)]


def scene_onboard():
    dur = 8.0
    code, rival, lap = "OKA", "MAR", 56
    PLAY0, SPEEDUP = 0.6, 3.2
    st = STARTS[code][lap]
    span_f = 0.30  # lap fraction shown

    def T_of(t):
        return st - 0.4 + max(0.0, t - PLAY0) * SPEEDUP

    ts = [i / 30 for i in range(int(dur * 30) + 1)]
    fr = [(t, frac_at(code, T_of(t)) - (lap - FIRST)) for t in ts]
    fr = [(t, min(max(f, 0.0), 1.0)) if f > -0.5 else (t, 0.0) for t, f in fr]
    # derive speed from the clock (consistent with lap times) + gear, throttle, brake from the profile
    spd, gear, thr, brk = [], [], [], []
    for t, f in fr:
        f = f if f >= 0 else f + 1
        spd.append((t, speed_at(f)))
        i = sample_idx(f)
        gear.append((t, DATA["gear"][i]))
        thr.append((t, DATA["throttle"][i]))
        brk.append((t, DATA["brake"][i]))
    # gear as steps
    gch = [gear[0]]
    for t, g in gear[1:]:
        if g != gch[-1][1]:
            gch.append((t, g))
    layers = backdrop()
    accent = COL[code]

    # --- speed dial
    cx, cy, R = 560, 590, 300
    a0, a1 = 140, 400
    vmax = 360
    layers.append(path(poly_d(arc_pts(cx, cy, R, a0, a1)), "#FFFFFF14", 26, enter={"preset": "fadeIn"}))
    for v in range(0, 361, 20):
        ang = math.radians(a0 + (a1 - a0) * v / vmax)
        big = v % 60 == 0
        r0, r1 = R - 34, R - (58 if big else 46)
        layers.append(path(poly_d([(cx + r0 * math.cos(ang), cy + r0 * math.sin(ang)), (cx + r1 * math.cos(ang), cy + r1 * math.sin(ang))]),
                           "#FFFFFF" + ("90" if big else "40"), 3 if big else 2, enter={"preset": "fadeIn", "delay": 0.1 + v / 1200}))
        if big:
            rl = R - 86
            layers.append(text(str(v), cx + rl * math.cos(ang), cy + rl * math.sin(ang), 20, MUTED, MONO, 600, "center",
                               enter={"preset": "fadeIn", "delay": 0.1 + v / 1200}))
    arc = poly_d(arc_pts(cx, cy, R, a0, a1, 180))
    layers.append(path(arc, accent, 26, stroke={"color": accent, "width": 26, "cap": "butt"}, trimEnd=r(spd[0][1] / vmax, 3),
                       glow={"radius": 22, "intensity": 0.9, "color": accent},
                       animate=[kf({"trimEnd": [(t, v / vmax) for t, v in spd]}, dur)]))
    layers.append(text("{value:0}", cx, cy - 20, 150, TEXT, MONO, 800, "center", value=r(spd[0][1]),
                       animate=[kf({"value": spd}, dur)]))
    layers.append(text("KM/H", cx, cy + 62, 24, MUTED, MONO, 700, "center", letterSpacing="0.2em"))
    # gear box
    layers.append(rect(cx, cy + 170, 120, 120, PANEL2, anchor="center", radius=6, stroke={"color": accent, "width": 2}))
    layers.append(text("GEAR", cx, cy + 128, 15, MUTED, MONO, 700, "center", letterSpacing="0.2em"))
    layers.append(text("{value:0}", cx, cy + 178, 84, accent, MONO, 800, "center", value=gch[0][1],
                       animate=[kf({"value": steps(gch, dur, tween=0.02)}, dur)]))
    # shift lights: 12 LEDs across the top of the dial, lit by speed within the gear band
    nled = 12
    for k in range(nled):
        lx = cx - 5.5 * 34 + k * 34
        colr = GREEN if k < 5 else (RED if k < 9 else PURPLE)
        lit = []
        for (t, v), (_, g) in zip(spd, gear):
            lo = [0, 95, 128, 160, 194, 230, 262, 296, 340][g - 1]
            hi = [0, 95, 128, 160, 194, 230, 262, 296, 340][g] if g < 8 else 340
            u = (v - lo) / max(1, hi - lo)
            lit.append((t, 1.0 if u * nled > k else 0.12))
        layers.append({"shape": "circle", "width": 20, "height": 20, "x": r(lx), "y": cy - R - 60, "fill": colr,
                       "opacity": 0.12, "animate": [kf({"opacity": lit}, dur)]})

    # --- throttle / brake bars
    for k, (lab, series, c) in enumerate((("THR", thr, GREEN), ("BRK", brk, RED))):
        bx = 960 + k * 70
        layers.append(rect(bx, 760, 44, 360, PANEL2, anchor="bottom"))
        layers.append(rect(bx, 760, 44, 360, c, anchor="bottom", scaleY=r(series[0][1], 3),
                           animate=[kf({"scaleY": series}, dur)]))
        layers.append(text(lab, bx, 790, 18, MUTED, MONO, 700, "center", letterSpacing="0.1em"))
        layers.append(text("{value:0}", bx, 380, 22, TEXT, MONO, 700, "center", value=0,
                           animate=[kf({"value": [(t, v * 100) for t, v in series]}, dur)]))

    # --- speed trace chart
    gx0, gx1, gy0, gy1 = 1110, W - M, 230, 560
    vmin_c, vmax_c = 80, 350

    def gxy(f, v):
        return gx0 + (gx1 - gx0) * f / span_f, gy1 - (gy1 - gy0) * (v - vmin_c) / (vmax_c - vmin_c)

    layers.append(rect(gx0 - 30, gy0 - 90, gx1 - gx0 + 60, gy1 - gy0 + 150, PANEL, opacity=0.85))
    layers.append(text("SPEED TRACE  ·  LAP 56  ·  S1", gx0, gy0 - 56, 20, TEXT, MONO, 700, "left", letterSpacing="0.08em"))
    layers.append(rect(gx1 - 250, gy0 - 66, 14, 4, accent))
    layers.append(text("OKA", gx1 - 228, gy0 - 64, 18, accent, MONO, 800, "left"))
    layers.append(rect(gx1 - 140, gy0 - 66, 14, 4, "#9AA4B5"))
    layers.append(text("MAR", gx1 - 118, gy0 - 64, 18, "#9AA4B5", MONO, 800, "left"))
    for v in (100, 150, 200, 250, 300):
        _, yy = gxy(0, v)
        layers.append(rect(gx0, yy, gx1 - gx0, 1, "#FFFFFF14"))
        layers.append(text(str(v), gx0 - 10, yy, 15, MUTED, MONO, 500, "right"))
    for k in range(0, 4):
        f = k * 0.1
        xx, _ = gxy(f, vmin_c)
        layers.append(text(f"{f * race.TRACK_M / 1000:.1f} KM", xx, gy1 + 22, 15, MUTED, MONO, 500, "center"))
    # rival ghost: same profile at the rival's pace (identical shape: what differs is time)
    ghost = [gxy(i / NS, SPEED[i]) for i in range(0, int(span_f * NS) + 1, 2)]
    layers.append(path(poly_d(ghost), "#9AA4B5", 2.5, opacity=0.7, animate=[{"from": {"trimEnd": 0}, "duration": 0.8, "ease": "outCubic"}]))
    # live trace (arc-length keyed trim)
    tr = [gxy(i / NS, SPEED[i]) for i in range(0, int(span_f * NS) + 1, 2)]
    cum = [0.0]
    for a, b in zip(tr[:-1], tr[1:]):
        cum.append(cum[-1] + math.dist(a, b))
    tot = cum[-1]

    def trim_of(f):
        k = min(len(cum) - 1, int(f * NS / 2))
        return cum[k] / tot

    layers.append(path(poly_d(tr), accent, 4, trimEnd=0, glow={"radius": 10, "intensity": 0.6, "color": accent},
                       animate=[kf({"trimEnd": [(t, trim_of(min(f, span_f))) for t, f in fr]}, dur)]))
    head = [(t, gxy(min(f, span_f), speed_at(f))) for t, f in fr]
    layers.append({"shape": "circle", "width": 14, "height": 14, "fill": "#fff", "glow": {"radius": 12, "color": accent},
                   "x": r(head[0][1][0]), "y": r(head[0][1][1]),
                   "animate": [kf({"x": [(t, p[0]) for t, p in head], "y": [(t, p[1]) for t, p in head]}, dur)]})

    # --- delta panel: time difference to the rival's lap 56 at the same distance
    lc = API["lap_clock"]
    delta = [(t, lc(code, lap, f) - lc(rival, lap, f)) for t, f in fr]
    dx, dy = 1110 - 30, 690
    layers.append(rect(dx, dy, 400, 170, PANEL, opacity=0.85))
    layers.append(text("DELTA TO MAR · LAP 56", dx + 24, dy + 32, 17, MUTED, MONO, 700, "left", letterSpacing="0.08em"))
    layers.append(text("{value:0.000}", dx + 24, dy + 100, 76, GREEN, MONO, 800, "left", value=0,
                       animate=[kf({"value": delta}, dur)]))
    # minimap
    mxf = map_xf(1680, 775, 0.27)
    layers.append(rect(1480, dy, W - M - 1480 + 30, 170, PANEL, opacity=0.85))
    layers.append(path(track_d(mxf, step=6), "#FFFFFF40", 4))
    layers.append(path(track_d(mxf, 0, span_f, 4), accent, 4, opacity=0.6))
    mp = [(t, track_point(f, mxf)) for t, f in fr]
    layers.append({"shape": "circle", "width": 16, "height": 16, "fill": accent, "stroke": {"color": "#fff", "width": 2},
                   "x": r(mp[0][1][0]), "y": r(mp[0][1][1]),
                   "animate": [kf({"x": [(t, p[0]) for t, p in mp], "y": [(t, p[1]) for t, p in mp]}, dur)]})

    layers.append(bug([(0, 56)], dur))
    layers.append(group([
        rect(W - M, 52, 210, 44, accent, anchor="topRight"),
        text("ONBOARD", W - M - 105, 74, 22, "#05070B", HEAD, 900, "center", letterSpacing="0.1em"),
        text("×3.2", W - M - 226, 74, 20, MUTED, MONO, 600, "right"),
    ], enter={"preset": "fadeLeft", "delay": 0.2}))
    layers.append(lower_third(code, "VANTOR  ·  P2  ·  +0.692 TO LEADER", 1.0, 6.8, x=M, y=H - 170))
    return {"id": "onboard", "duration": dur, "background": BG,
            "transition": {"type": "wipe", "direction": "left", "duration": 0.45},
            "layers": layers}


# ---- replay wipe (two halves straddling a cut) ------------------------------------
WIPE = {"duration": 0.4, "hold": 0.32, "colors": [PANEL2, "#8E1422", RED], "direction": "right", "slant": 14}


def replay_wipe_out(scene_dur):
    cover = WIPE["duration"] + 0.07 * 2
    at = scene_dur - cover
    return [{"component": "wipe", "props": dict(WIPE, at=r(at, 3))},
            text("REPLAY", W / 2, H / 2, 120, "#FFFFFF", HEAD, 900, "center", italic=True, letterSpacing="0.1em",
                 start=r(scene_dur - 0.12, 3), enter={"preset": "fadeIn", "duration": 0.08})]


def replay_wipe_in():
    cover = WIPE["duration"] + 0.07 * 2
    return [{"component": "wipe", "props": dict(WIPE, at=r(-cover, 3))},
            text("REPLAY", W / 2, H / 2, 120, "#FFFFFF", HEAD, 900, "center", italic=True, letterSpacing="0.1em",
                 end=0.14, exit={"preset": "fadeOut", "duration": 0.06})]


# ---- scene 4: overtake replay --------------------------------------------------------
def scene_overtake():
    dur = 8.0
    T_pass = DATA["overtakes"]["OKA_MAR"]["t"]
    t_pass = OT_PASS  # video time of the pass
    # piecewise playback: 1x, slow-mo 0.35x around the pass, 1x
    def T_of(t):
        a, b = t_pass - 0.9, t_pass + 1.0
        if t < a:
            return T_pass - 0.9 * 0.35 - (a - t)
        if t < b:
            return T_pass + (t - t_pass) * 0.35
        return T_pass + 1.0 * 0.35 + (t - b)

    f_pass = frac_at("OKA", T_pass) % 1.0
    scale = 4.2
    px, py = TRACK[sample_idx(f_pass)]
    xf = lambda x, y: (W / 2 + (x - px) * scale + 120, H / 2 + (y - py) * scale - 40)
    ts = [i / 60 for i in range(int(dur * 60) + 1)]
    layers = [{"shape": "rect", "width": W, "height": H, "x": W / 2, "y": H / 2, "fill": "radial-gradient(#101826, #05070B)"},
              {"component": "grid", "props": {"cell": 96, "color": "#FFFFFF07", "lineWidth": 1, "fade": 0.8}}]
    zoom_layers = []
    lo, hi = f_pass - 0.13, f_pass + 0.12
    d = track_d(xf, lo, hi, 1)
    zoom_layers += [path(d, "#151D2B", 168), path(d, "#2A3446", 148), path(d, "#FFFFFF22", 2, stroke={"color": "#FFFFFF30", "width": 2, "dash": [30, 30]})]
    # kerbs at the apex
    ax, ay = xf(*TRACK[sample_idx(0.215)])
    zoom_layers.append({"shape": "circle", "width": 30, "height": 30, "fill": RED, "x": r(ax), "y": r(ay), "opacity": 0.0})
    # braking boards
    for k, m in enumerate((150, 100, 50)):
        bf = 0.2 - m / race.TRACK_M - 0.012
        bx, by = xf(*TRACK[sample_idx(bf)])
        zoom_layers.append(text(str(m), bx, by + 150, 24, MUTED, MONO, 800, "center"))
        zoom_layers.append(rect(bx, by + 118, 3, 24, "#FFFFFF40", anchor="center"))
    for code in ("MAR", "OKA"):
        pts = [(t, xf(*track_point(frac_at(code, T_of(t)), lambda a, b: (a, b)))) for t in ts]
        off = 30 if code == "OKA" else -30  # lateral line: OKA on the inside
        # offset perpendicular to the track direction
        pp = []
        for t, (x, y) in pts:
            f = frac_at(code, T_of(t)) % 1.0
            i = sample_idx(f)
            a, b = TRACK[(i - 4) % NS], TRACK[(i + 4) % NS]
            nx, ny = -(b[1] - a[1]), (b[0] - a[0])
            ln = math.hypot(nx, ny) or 1
            pp.append((t, (x + nx / ln * off, y + ny / ln * off)))
        def at_T(T, off=off):
            f = frac_at(code, T) % 1.0
            x, y = xf(*track_point(f, lambda a, b: (a, b)))
            i = sample_idx(f)
            a2, b2 = TRACK[(i - 4) % NS], TRACK[(i + 4) % NS]
            nx, ny = -(b2[1] - a2[1]), (b2[0] - a2[0])
            ln = math.hypot(nx, ny) or 1
            return x + nx / ln * off, y + ny / ln * off

        # light trail: ghosts a few hundredths of a race-second behind
        for k in range(6, 0, -1):
            lag = 0.07 * k
            gp = [(t, at_T(T_of(t) - lag)) for t in ts[::2]]
            zoom_layers.append({"shape": "circle", "width": r(30 - 3 * k), "height": r(30 - 3 * k), "fill": COL[code],
                                "opacity": r(0.5 - 0.07 * k, 2), "x": r(gp[0][1][0]), "y": r(gp[0][1][1]),
                                "animate": [kf({"x": [(t, p[0]) for t, p in gp], "y": [(t, p[1]) for t, p in gp]}, dur)]})
        tag_y = -60 if code == "MAR" else 60
        car = [
            {"shape": "rect", "width": 86, "height": 34, "radius": 10, "fill": COL[code], "stroke": {"color": "#05070B", "width": 3},
             "glow": {"radius": 18, "intensity": 0.8, "color": COL[code]}},
            rect(0, tag_y, 62, 34, "#05070BDD", anchor="center", radius=4, stroke={"color": COL[code], "width": 2}),
            text(code, 0, tag_y, 20, TEXT, MONO, 800, "center"),
        ]
        if code == "MAR":
            car.append(group([rect(0, tag_y - 40, 140, 30, RED, anchor="center", radius=3),
                              text("LOCK-UP", 0, tag_y - 40, 18, "#fff", MONO, 800, "center", width=150)],
                             width=0, height=0, start=r(t_pass - 0.55), end=r(t_pass + 1.4),
                             enter={"preset": "popIn", "duration": 0.25}, exit={"preset": "fadeOut", "duration": 0.2}))
        # heading so the car points along the track
        rots = []
        prev = None
        for t, f in [(t, frac_at(code, T_of(t)) % 1.0) for t in ts[::6]]:
            i = sample_idx(f)
            a, b = TRACK[(i - 3) % NS], TRACK[(i + 3) % NS]
            ang = math.degrees(math.atan2(b[1] - a[1], b[0] - a[0]))
            if prev is not None:
                while ang - prev > 180:
                    ang -= 360
                while ang - prev < -180:
                    ang += 360
            prev = ang
            rots.append((t, ang))
        zoom_layers.append(group([car[0]], width=0, height=0, x=r(pp[0][1][0]), y=r(pp[0][1][1]),
                                 animate=[kf({"x": [(t, p[0]) for t, p in pp], "y": [(t, p[1]) for t, p in pp]}, dur),
                                          kf({"rotation": rots}, dur)]))
        zoom_layers.append(group(car[1:], width=0, height=0, x=r(pp[0][1][0]), y=r(pp[0][1][1]),
                                 animate=[kf({"x": [(t, p[0]) for t, p in pp], "y": [(t, p[1]) for t, p in pp]}, dur)]))
    for k in range(16):  # speed lines
        yk = 60 + (k * 389) % 960
        lw = 300 + (k * 137) % 500
        layers.append(rect(W + 200, yk, lw, 2 + k % 3, "#FFFFFF", anchor="left", opacity=0.05 + 0.02 * (k % 3),
                           animate=[{"from": {"x": W + 100 + (k * 211) % 900}, "to": {"x": -lw - 200}, "duration": 0.9 + 0.1 * (k % 5),
                                     "at": 0.0, "ease": "linear", "repeat": -1}]))
    layers.append(group(zoom_layers, animate=[{"from": {"scale": 0.95}, "to": {"scale": 1.12}, "duration": dur, "ease": "inOutSine"}]))

    # battle card (right): live speeds and gap
    def spd(code, t):
        e = 0.05
        return (frac_at(code, T_of(t) + e) - frac_at(code, T_of(t) - e)) / (2 * e) * race.TRACK_M * 3.6

    ts2 = [i / 20 for i in range(int(dur * 20) + 1)]
    cx0 = W - M - 520
    layers.append(rect(cx0, 150, 520, 300, PANEL, opacity=0.92))
    layers.append(rect(cx0, 150, 520, 44, PANEL2))
    layers.append(text("BATTLE FOR THE LEAD  ·  LAP 57", cx0 + 20, 172, 18, TEXT, MONO, 800, "left", letterSpacing="0.06em"))
    for k, code in enumerate(("MAR", "OKA")):
        yy = 230 + k * 74
        layers.append(rect(cx0 + 20, yy, 6, 54, COL[code], anchor="left"))
        layers.append(text(code, cx0 + 44, yy, 34, TEXT, HEAD, 900, "left"))
        layers.append(text(TEAM[code], cx0 + 130, yy, 16, MUTED, MONO, 600, "left"))
        sp = [(t, spd(code, t)) for t in ts2]
        layers.append(text("{value:0}", cx0 + 430, yy, 40, TEXT, MONO, 800, "right", value=r(sp[0][1]),
                           animate=[kf({"value": sp}, dur)]))
        layers.append(text("KM/H", cx0 + 440, yy + 6, 15, MUTED, MONO, 600, "left"))
    # interval: time for the chaser to reach the leader's current distance
    gaps = []
    for t in ts2:
        T = T_of(t)
        fa, fb = frac_at("MAR", T), frac_at("OKA", T)
        lead, chase = ("MAR", "OKA") if fa >= fb else ("OKA", "MAR")
        f_lead = max(fa, fb)
        lo2, hi2 = T, T + 3
        for _ in range(30):
            mid = (lo2 + hi2) / 2
            if frac_at(chase, mid) < f_lead:
                lo2 = mid
            else:
                hi2 = mid
        gaps.append((t, (lo2 - T) * (1 if lead == "MAR" else -1)))
    layers.append(text("INTERVAL", cx0 + 20, 410, 16, MUTED, MONO, 700, "left", letterSpacing="0.1em"))
    layers.append(text("{value:+0.000}", cx0 + 500, 410, 30, YELLOW, MONO, 800, "right", value=r(gaps[0][1], 3),
                       animate=[kf({"value": gaps}, dur)]))
    # pass banner
    layers.append(group([
        rect(W / 2 + 120, H - 300, 760, 96, COL["OKA"], anchor="center", skewX=-10),
        text("OKAFOR TAKES THE LEAD", W / 2 + 120, H - 300, 50, "#05070B", HEAD, 900, "center", italic=True),
    ], start=t_pass + 0.15, enter={"preset": "wipeIn", "direction": "right", "duration": 0.35, "ease": "outExpo"}))
    layers.append(group([
        rect(M, H - 150, 8, 86, RED),
        text("TURN 1", M + 32, H - 124, 44, TEXT, HEAD, 900, "left", italic=True, letterSpacing="0.04em"),
        text("BRAKING ZONE  ·  2ND GEAR HAIRPIN  ·  140 KM/H APEX", M + 34, H - 80, 18, MUTED, MONO, 700, "left", letterSpacing="0.08em"),
    ], enter={"preset": "fadeRight", "delay": 0.4, "duration": 0.5, "ease": "outExpo"}))
    # replay tag
    layers.append(group([
        rect(M, 52, 200, 44, RED, skewX=-12),
        text("REPLAY", M + 100, 74, 24, "#fff", HEAD, 900, "center", letterSpacing="0.14em", italic=True),
        {"shape": "circle", "width": 12, "height": 12, "x": M + 228, "y": 74, "fill": RED, "loop": "blink"},
        text("LAP 57  ·  TURN 1", M + 248, 74, 20, TEXT, MONO, 700, "left", letterSpacing="0.06em"),
    ]))
    slow = text("SLOW MOTION ×0.35", W - M, H - 60, 18, MUTED, MONO, 700, "right", letterSpacing="0.1em",
                start=r(t_pass - 0.9), end=r(t_pass + 1.0), enter={"preset": "fadeIn", "duration": 0.1}, exit={"preset": "fadeOut", "duration": 0.1})
    layers.append(slow)
    layers += replay_wipe_in()
    return {"id": "overtake", "duration": dur, "background": BG, "transition": "cut",
            "camera": {"shake": {"amount": 6, "frequency": 9, "at": r(t_pass - 0.1), "duration": 0.5},
                       "animate": [{"keyframes": {"zoom": [1, 1.06, 1.03]}, "at": r(t_pass - 0.05), "duration": 0.6, "ease": "outExpo"}]},
            "layers": layers}


# ---- scene 5: fastest lap sting ---------------------------------------------------------
def scene_fastest():
    dur = 3.6
    fl = DATA["fastest_lap"]
    code = fl["code"]
    secs = DATA["sector_times"][code][str(fl["lap"])]
    layers = [
        {"shape": "rect", "width": W, "height": H, "x": W / 2, "y": H / 2, "fill": "linear-gradient(120deg, #1A0633, #05070B)"},
    ]
    for k in range(7):  # speed streaks
        y = 140 + k * 130
        layers.append(rect(-400, y, 900 + 120 * (k % 3), 6 + 4 * (k % 2), PURPLE, opacity=0.35 + 0.1 * (k % 3), anchor="left",
                           animate=[{"from": {"x": -1200}, "to": {"x": W + 400}, "duration": 1.2 + 0.15 * (k % 4), "at": 0.05 * k, "ease": "inOutCubic"}]))
    layers += [
        rect(M + 40, 330, 14, 380, PURPLE, enter={"preset": "wipeIn", "direction": "down", "duration": 0.4}),
        text("FASTEST LAP", M + 90, 400, 120, TEXT, HEAD, 900, "left", italic=True, split="chars",
             enter={"preset": "fadeLeft", "stagger": 0.025, "duration": 0.4, "ease": "outExpo"}),
        text("{value}", M + 96, 540, 140, PURPLE, MONO, 800, "left", value=0),
        text(f"{NAME[code].upper()}  ·  {TEAM[code]}  ·  LAP {fl['lap']}", M + 96, 650, 30, TEXT, MONO, 700, "left",
             letterSpacing="0.06em", enter={"preset": "fadeUp", "delay": 0.4}),
    ]
    # replace the {value} counter with a lap-time string reveal (mono, scrambled)
    # a timer settling: minutes fixed, seconds count up to the lap time (digits only)
    mins = int(fl["time"] // 60)
    secs_v = fl["time"] - 60 * mins
    layers[-2] = text(f"{mins}:{{value:00.000}}", M + 96, 540, 140, PURPLE, MONO, 800, "left", value=r(secs_v, 3),
                      glow={"radius": 30, "intensity": 0.9, "color": PURPLE},
                      animate=[{"from": {"value": r(secs_v - 6.5, 3)}, "at": 0.1, "duration": 0.45, "ease": "outExpo"}])
    for k, s in enumerate(secs):
        x = 1240 + k * 0
        y = 360 + k * 110
        layers.append(group([
            rect(1240, y, 540, 88, "#140A26"),
            rect(1240, y, 10, 88, PURPLE),
            text(f"S{k + 1}", 1276, y + 44, 30, MUTED, MONO, 800, "left"),
            text(f"{s:.3f}", 1756, y + 44, 48, TEXT, MONO, 800, "right"),
        ], enter={"preset": "fadeLeft", "delay": 0.5 + 0.12 * k, "duration": 0.5, "ease": "outExpo"}))
    layers.append({"component": "sparks", "props": {"bursts": [0.3], "origin": [0.3, 0.45], "count": 90, "colors": ["#FFFFFF", PURPLE], "power": 1.2}})
    return {"id": "fastest", "duration": dur, "background": BG,
            "transition": {"type": "glitch", "duration": 0.25, "intensity": 0.6}, "layers": layers}


# ---- scene 6: analysis --------------------------------------------------------------------
def scene_analysis():
    dur = 8.0
    lap = 57
    st = DATA["sector_times"]
    a, b = "OKA", "MAR"
    best = [min(st[c][str(l)][k] for c in CODES for l in range(50, 59)) for k in range(3)]
    layers = backdrop()
    layers.append(bug([(0, 57)], dur))
    layers.append(text("LAP 57  ·  HEAD TO HEAD", W - M, 74, 24, TEXT, HEAD, 800, "right", letterSpacing="0.08em",
                       enter={"preset": "fadeLeft", "delay": 0.1}))
    # sector comparison (left)
    x0, y0, pw = M, 170, 780
    layers.append(rect(x0, y0, pw, 760, PANEL, opacity=0.9, enter={"preset": "fadeIn", "duration": 0.3}))
    layers.append(text("SECTOR TIMES", x0 + 32, y0 + 44, 22, TEXT, MONO, 800, "left", letterSpacing="0.1em"))
    for k2, (lab, c) in enumerate((("OVERALL BEST", PURPLE), ("PERSONAL BEST", GREEN), ("SLOWER", YELLOW))):
        layers.append(rect(x0 + 270 + k2 * 175, y0 + 44, 12, 12, c, anchor="left"))
        layers.append(text(lab, x0 + 288 + k2 * 175, y0 + 44, 13, MUTED, MONO, 700, "left"))
    for k in range(3):
        yk = y0 + 110 + k * 210
        layers.append(text(f"SECTOR {k + 1}", x0 + 32, yk, 18, MUTED, MONO, 700, "left", letterSpacing="0.12em",
                           enter={"preset": "fadeIn", "delay": 0.2 + 0.3 * k}))
        ta, tb = st[a][str(lap)][k], st[b][str(lap)][k]
        lo_v = min(ta, tb) - 1.2
        hi_v = max(ta, tb) + 0.2
        for j, (code, tv) in enumerate(((a, ta), (b, tb))):
            pb = min(st[code][str(l)][k] for l in range(50, lap + 1))
            col = PURPLE if tv <= best[k] + 1e-6 else (GREEN if tv <= pb + 1e-6 else YELLOW)
            yy = yk + 40 + j * 62
            frac = (tv - lo_v) / (hi_v - lo_v)
            bw = 420 * frac
            layers.append(text(code, x0 + 32, yy + 24, 26, TEXT, HEAD, 900, "left"))
            layers.append(rect(x0 + 110, yy + 6, 6, 36, COL[code]))
            layers.append(rect(x0 + 124, yy + 6, r(bw), 36, col, scaleX=1,
                               anchor="topLeft", animate=[{"from": {"width": 0}, "at": 0.3 + 0.3 * k + 0.1 * j, "duration": 0.7, "ease": "outExpo"}]))
            layers.append(text("{value:0.000}", x0 + pw - 32, yy + 24, 30, TEXT, MONO, 800, "right", value=r(tv, 3),
                               start=r(0.3 + 0.3 * k + 0.1 * j), enter={"preset": "countUp", "duration": 0.7}))
        dlt = ta - tb
        layers.append(text(f"{dlt:+.3f}", x0 + pw - 32, yk, 20, GREEN if dlt < 0 else RED, MONO, 800, "right",
                           enter={"preset": "fadeIn", "delay": 0.9 + 0.3 * k}))
    # lap time chart (right)
    laps = list(range(48, 59))
    cx0 = x0 + pw + 40
    cw = W - M - cx0
    layers.append(rect(cx0, y0, cw, 760, PANEL, opacity=0.9, enter={"preset": "fadeIn", "duration": 0.3, "delay": 0.2}))
    layers.append(text("LAP TIMES  ·  LAPS 48–58", cx0 + 32, y0 + 44, 22, TEXT, MONO, 800, "left", letterSpacing="0.1em"))
    lt = DATA["lap_times"]
    series = [[lt[c][str(l)] for l in laps] for c in ("OKA", "MAR", "IVE")]
    layers.append({"component": "lineChart", "x": cx0 + 20, "y": y0 + 90, "anchor": "topLeft", "width": cw - 40, "height": 640,
                   "props": {"series": series, "names": ["OKA", "MAR", "IVE"], "labels": [f"L{l}" for l in laps],
                             "colors": [COL["OKA"], COL["MAR"], COL["IVE"]], "min": 91.0, "max": 92.8, "smooth": False,
                             "dots": False, "area": False, "width": 5, "delay": 0.6, "duration": 3.2, "stagger": 0.25,
                             "format": {"decimals": 3}, "fontSize": 22}})
    fl = DATA["fastest_lap"]
    return {"id": "analysis", "duration": dur, "background": BG,
            "transition": {"type": "slide", "direction": "up", "duration": 0.5, "ease": "inOutQuart"}, "layers": layers}


# ---- scene 7: winner ---------------------------------------------------------------------
def scene_winner():
    dur = 7.0
    cl = DATA["classification"]
    w = cl[0]["code"]
    layers = [{"shape": "rect", "width": W, "height": H, "x": W / 2, "y": H / 2, "fill": "radial-gradient(#1B1208, #05070B)"}]
    # chequered band
    sq = 40
    band = []
    for i in range(0, 56):
        for j in range(3):
            if (i + j) % 2 == 0:
                band.append(rect(i * sq, j * sq, sq, sq, "#FFFFFF"))
    layers.append(group(band, x=-200, y=H - 230, anchor="topLeft", width=W, height=sq * 3, opacity=0.08, skewY=-4,
                        animate=[{"from": {"x": -260}, "to": {"x": -20}, "duration": dur, "ease": "linear"}]))
    layers.append(group([b2 for b2 in band], x=-200, y=110, anchor="topLeft", width=W, height=sq * 3, opacity=0.08, skewY=-4,
                        animate=[{"from": {"x": -20}, "to": {"x": -260}, "duration": dur, "ease": "linear"}]))
    layers.append(text("WINNER  ·  KESSRA GRAND PRIX", W / 2, 300, 30, RED, MONO, 800, "center", letterSpacing="0.3em",
                       enter={"preset": "fadeIn", "delay": 0.3}))
    layers.append({"component": "text-3d", "x": W / 2, "y": 470, "width": 1700, "height": 380,
                   "props": {"text": NAME[w].split()[1].upper(), "gradient": ["#FFFFFF", COL[w], "#FFFFFF"],
                             "weight": 900, "tracking": 0.04, "depth": 0.22, "intro": "flyIn", "stagger": 0.06, "delay": 0.1,
                             "sway": 5, "shine": 1.2}})
    for k, row in enumerate(cl[:3]):
        y = 720 + k * 74
        c = row["code"]
        gap = "WINNER" if k == 0 else f"+{row['gap']:.3f}"
        layers.append(group([
            rect(W / 2 - 400, y, 800, 62, PANEL),
            rect(W / 2 - 400, y, 62, 62, [YELLOW, "#C9D1DC", "#D08A4E"][k]),
            text(str(k + 1), W / 2 - 369, y + 31, 34, "#05070B", MONO, 900, "center"),
            rect(W / 2 - 322, y + 12, 6, 38, COL[c]),
            text(NAME[c].upper(), W / 2 - 300, y + 31, 28, TEXT, HEAD, 800, "left"),
            text(TEAM[c], W / 2 + 120, y + 31, 18, MUTED, MONO, 700, "left"),
            text(gap, W / 2 + 384, y + 31, 26, TEXT, MONO, 800, "right"),
        ], enter={"preset": "fadeUp", "delay": 1.4 + 0.15 * k, "duration": 0.5, "ease": "outExpo"}))
    total = cl[0]["time"]
    layers.append(text(f"58 LAPS  ·  296.496 KM  ·  FASTEST LAP {fmt_lap(DATA['fastest_lap']['time'])} ({DATA['fastest_lap']['code']})",
                       W / 2, 1000, 20, MUTED, MONO, 600, "center", letterSpacing="0.08em", enter={"preset": "fadeIn", "delay": 2.1}))
    layers.append({"component": "confetti", "props": {"mode": "cannons", "at": 0.9, "count": 160, "colors": [COL[w], "#FFFFFF", RED], "power": 1.0}})
    return {"id": "winner", "duration": dur, "background": BG,
            "transition": {"type": "iris", "duration": 0.6}, "layers": layers}


def build():
    onboard = scene_onboard()
    onboard["layers"] += replay_wipe_out(onboard["duration"])
    spec = {
        "size": "16:9", "fps": FPS,
        "theme": {"colors": {"primary": RED, "accent": RED, "bg": BG, "surface": PANEL, "text": TEXT, "muted": MUTED},
                  "fonts": {"heading": "Inter Display", "body": "Inter", "mono": "JetBrains Mono"}},
        "background": BG,
        "scenes": [scene_open(), scene_map(), onboard, scene_overtake(), scene_fastest(), scene_analysis(), scene_winner()],
    }
    # scene start times (transitions overlap scenes)
    starts, t = [], 0.0
    for i, sc in enumerate(spec["scenes"]):
        tr = sc.get("transition")
        td = 0.0 if i == 0 or tr in (None, "cut") else (tr.get("duration", 0.5) if isinstance(tr, dict) else 0.5)
        t -= td
        starts.append(t)
        t += sc["duration"]
    S = dict(zip([sc["id"] for sc in spec["scenes"]], starts))
    ive_pass = S["map"] + MAP_IVE_PASS
    spec["audio"] = [
        {"src": "score.mp3", "volume": 0.67, "fadeOut": 1.0},
        {"sfx": "whoosh", "at": r(S["map"] - 0.05), "volume": 0.35},
        {"sfx": "tick", "at": r(ive_pass), "volume": 0.42},
        {"sfx": "pop", "at": r(ive_pass + 0.1), "volume": 0.28},
        {"sfx": "swoosh", "at": r(S["onboard"] - 0.05), "volume": 0.32},
        {"sfx": "pop", "at": r(S["onboard"] + 1.0), "volume": 0.24},
        {"sfx": "whoosh", "at": r(S["overtake"] - 0.55), "volume": 0.42},
        {"sfx": "swoosh", "at": r(S["overtake"] + 0.3), "volume": 0.32},
        {"sfx": "impact", "at": r(S["overtake"] + OT_PASS), "volume": 0.39},
        {"sfx": "swoosh", "at": r(S["overtake"] + OT_PASS + 0.15), "volume": 0.28},
        {"sfx": "chime", "at": r(S["fastest"] + 0.1), "volume": 0.35},
        {"sfx": "whoosh", "at": r(S["analysis"] - 0.1), "volume": 0.28},
        {"sfx": "success", "at": r(S["winner"] + 0.4), "volume": 0.3},
    ]
    with open(os.path.join(HERE, "spec.json"), "w") as f:
        json.dump(spec, f, separators=(",", ":"))
    print("wrote spec.json", os.path.getsize(os.path.join(HERE, "spec.json")) // 1024, "KB")


if __name__ == "__main__":
    build()
