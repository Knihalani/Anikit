"""Hand-inked kestrel silhouettes as SVG path data (y down, facing right).

Local origin = the bird's eye. Everything is drawn as smooth closed outlines
(Catmull-Rom -> cubic Bezier) with optional sharp corners, plus "scratched"
clear lines (McLaren-style scratch-on-ink detail).
"""
import math, random

def _rot(p, deg):
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    return (p[0] * c - p[1] * s, p[0] * s + p[1] * c)

def smooth_closed(pts, sharp=(), tension=0.5):
    """pts: list of (x,y). sharp: indices with zero-length handles."""
    n = len(pts)
    out = [f"M{pts[0][0]:.1f} {pts[0][1]:.1f}"]
    for i in range(n):
        p0, p1, p2, p3 = pts[i - 1], pts[i], pts[(i + 1) % n], pts[(i + 2) % n]
        k = tension / 3 * 2
        c1 = (p1[0] + (p2[0] - p0[0]) * k / 2, p1[1] + (p2[1] - p0[1]) * k / 2)
        c2 = (p2[0] - (p3[0] - p1[0]) * k / 2, p2[1] - (p3[1] - p1[1]) * k / 2)
        if i in sharp:
            c1 = p1
        if (i + 1) % n in sharp:
            c2 = p2
        out.append(f"C{c1[0]:.1f} {c1[1]:.1f} {c2[0]:.1f} {c2[1]:.1f} {p2[0]:.1f} {p2[1]:.1f}")
    out.append("Z")
    return " ".join(out)

def open_path(pts):
    s = f"M{pts[0][0]:.1f} {pts[0][1]:.1f}"
    for i in range(1, len(pts) - 1):
        mx = (pts[i][0] + pts[i + 1][0]) / 2
        my = (pts[i][1] + pts[i + 1][1]) / 2
        s += f" Q{pts[i][0]:.1f} {pts[i][1]:.1f} {mx:.1f} {my:.1f}"
    s += f" L{pts[-1][0]:.1f} {pts[-1][1]:.1f}"
    return s

# ---- parts (front-on hover: wings out both sides, fanned tail) ----------

_B = [(-4, -16), (8, -15), (15, -8), (19, 1), (17, 8), (12, 5), (9, 12), (13, 24),
      (24, 44), (22, 80), (15, 104), (11, 114), (-11, 114), (-15, 104), (-22, 80),
      (-24, 44), (-15, 24), (-15, 4), (-12, -9)]
EYE = (5, -5)
BODY = [(x - EYE[0], y - EYE[1]) for x, y in _B]
BODY_SHARP = (4,)

def tail(spread):
    hw = 20 + 34 * spread
    pts = [(11, 108), (hw, 206), (hw * 0.55, 218), (0, 221), (-hw * 0.55, 218), (-hw, 206), (-11, 108)]
    return [(x - EYE[0], y - EYE[1]) for x, y in pts]

# wing in (u along, v toward leading edge)
WING_UV = [(0, 10), (38, 22), (88, 18), (160, 7), (226, 0), (206, -10), (188, -19),
           (170, -18), (150, -28), (112, -40), (60, -46), (10, -38)]
WING_SHARP = (4, 6, 8)

def _wing_pts(shoulder, a_deg, length, chord, bend, uv):
    a = math.radians(a_deg)
    d = (math.cos(a), -math.sin(a))
    n = (d[1], -d[0])
    out = []
    for u, v in uv:
        u2, v2 = u * length, v * chord
        if u > 88 and bend:
            du, dv = _rot((u2 - 88 * length, v2), -bend)
            u2, v2 = 88 * length + du, dv
        out.append((shoulder[0] + d[0] * u2 + n[0] * v2, shoulder[1] + d[1] * u2 + n[1] * v2))
    return out

SCR_UV = [[(100, -6), (150, -10), (178, -8)], [(104, -16), (148, -18), (168, -16)],
          [(110, -26), (140, -27), (152, -24)], [(20, -8), (60, -16), (96, -10)]]

def bird(pose, jitter=0.0, seed=0, scale=0.82, tilt=0):
    rnd = random.Random(seed)
    J = lambda pts: [(x + rnd.uniform(-jitter, jitter), y + rnd.uniform(-jitter, jitter)) for x, y in pts]
    T = lambda pts: [(_rot(p, tilt)[0] * scale, _rot(p, tilt)[1] * scale) for p in pts]
    a, L, C, bend = pose["a"], pose.get("length", 1), pose.get("chord", 1), pose.get("bend", 0)
    sh = (20 - EYE[0], 42 - EYE[1])
    shl = (-20 - EYE[0], 42 - EYE[1])
    right = _wing_pts(sh, a, L, C, bend, WING_UV)
    left = [(2 * shl[0] - x + 0, y) for x, y in _wing_pts((shl[0], shl[1]), a + pose.get("asym", 4), L * 0.93, C, bend, WING_UV)]
    # mirror: computed as a right wing around shl then reflected about x = shl.x
    parts = {
        "far": smooth_closed(T(J(left)), WING_SHARP),
        "tail": smooth_closed(T(J(tail(pose.get("spread", 0.8)))), (2, 3, 4)),
        "body": smooth_closed(T(J(BODY)), BODY_SHARP),
        "near": smooth_closed(T(J(right)), WING_SHARP),
    }
    lines = []
    for l in SCR_UV:
        lines.append(_wing_pts(sh, a, L, C, bend, l))
        lines.append([(2 * shl[0] - x, y) for x, y in _wing_pts(shl, a + pose.get("asym", 4), L * 0.93, C, bend, l)])
    hw = 20 + 34 * pose.get("spread", 0.8)
    lines.append([(x - EYE[0], y - EYE[1]) for x, y in [(hw * 0.8, 196), (0, 203), (-hw * 0.8, 196)]])
    lines.append([(-1, 4), (-2, 13), (0, 22)])  # the tear stripe, scratched
    parts["scratch"] = " ".join(open_path(T(J(l))) for l in lines)
    return parts

def stoop(seed=0, jitter=0.0):
    """wings tucked, head down: a falcon's stoop (the mark). Beak tip at (0,0)."""
    rnd = random.Random(seed)
    J = lambda pts: [(x + rnd.uniform(-jitter, jitter), y + rnd.uniform(-jitter, jitter)) for x, y in pts]
    body = [(0, 0), (5, -4), (8, -11), (11, -22), (13, -40), (12, -62), (9, -80), (7, -88),
            (-7, -88), (-9, -80), (-12, -62), (-13, -40), (-11, -22), (-8, -11), (-5, -4)]
    tail = [(6, -84), (9, -118), (0, -124), (-9, -118), (-6, -84)]
    wing_r = [(10, -30), (19, -64), (22, -102), (17, -138), (12, -96), (7, -60)]
    wing_l = [(-10, -30), (-7, -60), (-12, -96), (-17, -138), (-22, -102), (-19, -64)]
    return {
        "body": smooth_closed(J(body), (0,)),
        "tail": smooth_closed(J(tail), ()),
        "wr": smooth_closed(J(wing_r), (3,)),
        "wl": smooth_closed(J(wing_l), (3,)),
    }

HOVER = [
    dict(a=42, spread=0.85, bend=-14),
    dict(a=22, spread=0.9, bend=-8),
    dict(a=2, spread=0.95, bend=4),
    dict(a=-18, spread=0.95, bend=14, chord=0.95),
    dict(a=4, spread=0.9, bend=10, length=0.96),
    dict(a=28, spread=0.85, bend=-4, length=0.98),
]
FOLD = [
    dict(a=52, spread=0.6, bend=-24, length=0.88, chord=0.9),
    dict(a=60, spread=0.45, bend=-28, length=0.82, chord=0.84),
    dict(a=60, spread=0.3, bend=-20, length=0.74, chord=0.78),
]
