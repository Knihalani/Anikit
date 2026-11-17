"""EVERY FRAME: the world, as functions of time.

Single source of truth for the film's layout, timing and camera. `previz.py`
draws a grey-box animatic from it; `build.py` turns it into the anikit-video
spec. Units: metres and seconds. Y up, floor at y = 0. The strip runs along +X
(away from the machine); panes face +Z (towards the camera side).
"""

from __future__ import annotations

import math

FPS = 60
DURATION = 72.0
W, H = 1920, 1080

# --- Beat grid: 120 BPM ---------------------------------------------------
BPM = 120
BEAT = 60.0 / BPM          # 0.5 s
BAR = 4 * BEAT             # 2 s

# --- Objects ---------------------------------------------------------------
PANE_W, PANE_H, PANE_T = 1.6, 0.9, 0.008
PANE_Y = PANE_H / 2 + 0.02          # panes stand just above the floor
PITCH = 1.7                          # pane centre spacing along the strip
LOCK_SPEED = PITCH * FPS             # m/s: one pane per output frame = the film plays
FRAMES = 3600                        # the Sundial film: 60 s at 60 fps
BROWSER_FRAMES = 175                 # 3,600 x 3.37 s / 69.14 s (BENCHMARKS.md (d))

MACHINE = dict(x0=-1.0, x1=1.0, y0=0.0, y1=1.1, z0=-0.4, z1=0.4)
X_IN = 0.12                          # pane centre when n - j == 0 (inside the machine)
BROWSER_Z = -9.0                     # the browser line runs parallel, further back

# --- Timeline (seconds) ------------------------------------------------------
T = dict(
    flip=6.0,          # request pane turns over (spec on the back)
    enter0=12.5, enter1=14.5,   # pane slides into the machine
    led=14.5,
    frame1_0=15.5, frame1_1=16.5,
    stream0=18.0, stream1=28.5,  # 2..3600 frames
    ramp0=40.5, lock0=43.0, lock1=49.0, brake1=50.0,
    fold0=50.0, fold1=52.0,      # strip folds into a stack (lands on 52.0)
    crush0=52.5, crush1=54.0,    # stack compresses into a block
    shrink0=54.5, shrink1=56.3,  # block shrinks into the tile
    insert0=56.9, insert1=58.0,  # tile slides into the phone
    screen=59.0,
    logo0=66.0, logo1=68.0,
)


def clamp(x, a=0.0, b=1.0):
    return a if x < a else b if x > b else x


def smooth(x):
    x = clamp(x)
    return x * x * (3 - 2 * x)


def smoother(x):
    x = clamp(x)
    return x * x * x * (x * (6 * x - 15) + 10)


def ease_out(x, p=3):
    x = clamp(x)
    return 1 - (1 - x) ** p


def ease_in(x, p=3):
    return clamp(x) ** p


def seg(t, a, b):
    return clamp((t - a) / (b - a)) if b > a else float(t >= a)


def lerp(a, b, u):
    return a + (b - a) * u


def lerp3(a, b, u):
    return tuple(lerp(a[i], b[i], u) for i in range(3))


def spring(x, zeta=0.45, w=9.0):
    """Unit step response of an under-damped spring, x in seconds."""
    if x <= 0:
        return 0.0
    wd = w * math.sqrt(1 - zeta * zeta)
    return 1 - math.exp(-zeta * w * x) * (math.cos(wd * x) + zeta * w / wd * math.sin(wd * x))


# --- Frame emission ----------------------------------------------------------

def emitted(t: float) -> float:
    """Continuous count of frames out of the machine (n). Frame j is fully out at n = j + 1."""
    if t < T["frame1_0"]:
        return 0.0
    if t < T["stream0"]:
        return smoother(seg(t, T["frame1_0"], T["frame1_1"]))
    # 2, 3, 4 on the beat (18.0, 18.5, 19.0), then an accelerating torrent to 3600 at stream1.
    s0 = T["stream0"]
    if t < s0 + 1.5:
        k = (t - s0) / BEAT                 # 0..3
        i = math.floor(k)
        return 1 + i + smoother((k - i) / 0.6)
    u = seg(t, s0 + 1.5, T["stream1"])
    # n'' grows like a whir; ease-in power then a soft landing.
    v = u ** 2.6 * (1 - 0.15 * (1 - u))
    n = 4 + (FRAMES - 4) * v
    if u >= 1:
        return float(FRAMES)
    return n


BROWSER_RATE = BROWSER_FRAMES * PITCH / (T["stream1"] - T["stream0"])   # cart speed, m/s


def browser_x(t):
    return machine_x(browser_emitted(t))


def browser_emitted(t: float) -> float:
    """The fastest browser setup, at the same time scale: 175 frames when ours finishes, and it keeps going."""
    a, b = T["stream0"], T["stream1"]
    return max(0.0, BROWSER_FRAMES * (t - a) / (b - a))


def pane_x(j: int, n: float = 0.0) -> float:
    """Frame j stands still where the machine left it (frame 0 where the spec went in)."""
    return j * PITCH - 0.15


def machine_x(n: float) -> float:
    """Machine centre: it rolls forward one pitch per frame, revealing the pane behind it."""
    return n * PITCH


# --- Fold (shot 7) -------------------------------------------------------------
# The camera brakes beside pane J; every pane rushes to it, turns 90 degrees and
# stacks face to face, then the stack crushes into a block and the block into a tile.

STACK_STEP = 0.012


def fold_centre():
    return cam_pos(T["brake1"])[0]


def pane_pose_fold(j: int, t: float, xc: float, jc: int):
    """(x, y, z, yaw_deg, sx) of strip pane j during the fold. sx scales the stack length."""
    x0 = pane_x(j)
    xs = xc - (j - jc) * STACK_STEP      # stacked position (x decreases with j, like the strip)
    d = abs(j - jc)
    delay = 0.35 * (1 - math.exp(-d / 400))
    u = smoother(seg(t, T["fold0"] + delay, T["fold1"] - 0.05))
    x = lerp(x0, xs, u)
    yaw = 90 * smoother(seg(t, T["fold0"] + delay * 0.5, T["fold0"] + delay + 0.7))
    # crush: stack length 3600*0.012 = 43 m -> 0.3 m
    c = smoother(seg(t, T["crush0"], T["crush1"]))
    x = xc + (x - xc) * lerp(1.0, 0.3 / (FRAMES * STACK_STEP), c)
    return x, PANE_Y, 0.0, yaw


def block_scale(t):
    """Uniform scale of the crushed block as it shrinks into the tile (1 -> tile)."""
    return lerp(1.0, 0.06, smoother(seg(t, T["shrink0"], T["shrink1"])))


PHONE_OFFSET = (-0.9, 0.0, 0.9)   # relative to the fold centre


# --- Camera --------------------------------------------------------------------
# Keys: (t, pos, target, focal_mm, focus_dist). Cubic Hermite with Catmull-Rom
# tangents unless a velocity is given. The lock segment is exactly linear.

CAM_Z = 2.4                    # camera distance from the strip during the run
CAM_LOCK_Y = 0.55
CAM_LOCK_X0 = 0.0              # filled in below (start of lock)

REQ_POS = (-5.0, 1.05, 0.0)    # the request pane (shots 1-2)
LOGO_Z = 22.0                  # the logo stands on the open side of the studio, facing -Z


def _keys():
    x_lock0 = 600.0 + (BROWSER_RATE + LOCK_SPEED) / 2 * (T["lock0"] - 39.0) * 0.98
    x_lock1 = x_lock0 + LOCK_SPEED * (T["lock1"] - T["lock0"])
    x_brake = x_lock1 + LOCK_SPEED * (T["brake1"] - T["lock1"]) * 0.5
    globals()["CAM_LOCK_X0"] = x_lock0
    V = LOCK_SPEED
    rx, ry, rz = REQ_POS
    look_strip = (0.0, -0.05, -CAM_Z)
    return [
        # shot 1: push in on the request
        dict(t=0.0, pos=(rx + 0.3, ry + 0.08, 4.6), tgt=(rx, ry, 0), f=60, fd=4.6),
        dict(t=6.0, pos=(rx + 0.05, ry, 3.4), tgt=(rx, ry, 0), f=60, fd=3.4),
        # shot 2: orbit and pull back as the plan appears; the type stands to the right
        dict(t=10.5, pos=(rx + 2.4, ry + 0.25, 4.2), tgt=(rx + 1.0, ry - 0.25, 0), f=42, fd=4.3),
        # shot 3: the pane drops into the machine like toast; the machine rolls on and leaves frame 1 standing
        dict(t=13.5, pos=(-1.8, 1.6, 4.6), tgt=(0.0, 0.8, 0), f=40, fd=4.8),
        dict(t=17.0, pos=(-1.4, 0.85, 3.9), tgt=(0.4, 0.5, 0), f=40, fd=3.9),
        dict(t=19.5, pos=(-2.6, 1.0, 4.6), tgt=(2.0, 0.5, 0), f=35, fd=4.8),
        # shot 4: rise and chase the machine as it races off laying the strip
        dict(t=24.0, pos=(12.0, 6.0, 12.0), tgt=(70.0, 0.3, -3.0), f=30, fd=30),
        dict(t=29.0, pos=(140.0, 28.0, 40.0), tgt=(420.0, 0.0, -6.0), f=30, fd=120),
        # shot 5: drop down beside our finished strip and catch the browser cart, still working
        dict(t=33.0, pos=(browser_x(33.0) - 40.0, 3.2, 9.0), tgt=(browser_x(33.0) + 10.0, 0.5, -6.0), f=32, fd=30),
        dict(t=39.0, pos=(browser_x(39.0) - 14.0, 2.6, 8.0), tgt=(browser_x(39.0) + 12.0, 0.5, -6.0), f=32, fd=20,
             v=(BROWSER_RATE, 0.0, 0.0)),
        # shot 6: swoop down beside the strip, accelerate to lock speed (the film plays), brake
        dict(t=T["lock0"], pos=(x_lock0, CAM_LOCK_Y, CAM_Z), dir=look_strip, f=35, fd=CAM_Z, v=(V, 0, 0)),
        dict(t=T["lock1"], pos=(x_lock1, CAM_LOCK_Y, CAM_Z), dir=look_strip, f=35, fd=CAM_Z, v=(V, 0, 0)),
        dict(t=T["brake1"], pos=(x_brake, CAM_LOCK_Y + 0.1, CAM_Z + 0.4), dir=(0.0, -0.1, -CAM_Z), f=35, fd=CAM_Z, v=(0, 0, 0)),
        # shot 7: hold while the strip folds, then come down onto the shrinking block / tile
        dict(t=53.0, pos=(x_brake + 0.4, 0.9, CAM_Z + 1.2), tgt=(x_brake, 0.47, 0), f=35, fd=3.4),
        dict(t=56.0, pos=(x_brake + 0.35, 0.32, 1.45), tgt=(x_brake, 0.06, 0.0), f=50, fd=1.5, still=True),
        # shot 8: follow the tile into the phone (macro), hold on the lit screen
        dict(t=58.2, pos=(x_brake - 0.45, 0.2, 1.4), tgt=(x_brake - 0.82, 0.09, 0.8), f=60, fd=0.68, still=True),
        dict(t=60.6, pos=(x_brake - 0.68, 0.17, 1.38), tgt=(x_brake - 0.9, 0.09, 0.9), f=60, fd=0.52, still=True),
        # ... rise and look back down the other lane: the browser cart is still rendering
        dict(t=63.2, pos=(x_brake + 4.0, 4.5, 9.0), dir=(browser_x(63.2) - x_brake - 4.0, -3.9, BROWSER_Z - 9.0), f=110, fd=250),
        # ... then turn away from the lanes to the line of type, and on to the logo
        dict(t=65.8, pos=(x_brake + 0.5, 1.5, 3.5), dir=(0.0, -0.08, 1.0), f=40, fd=7.0),
        dict(t=70.0, pos=(x_brake, 1.35, LOGO_Z - 8.0), tgt=(x_brake, 0.55, LOGO_Z), f=40, fd=8.0),
        dict(t=72.0, pos=(x_brake, 1.3, LOGO_Z - 7.5), tgt=(x_brake, 0.55, LOGO_Z), f=40, fd=7.5),
    ]


KEYS = _keys()
for _k in KEYS:
    if "dir" in _k:
        _m = math.sqrt(sum(c * c for c in _k["dir"]))
        _k["tgt"] = tuple(_k["pos"][i] + 3 * _k["dir"][i] / _m for i in range(3))
LOGO_POS = (KEYS[-1]["pos"][0], 0.0, LOGO_Z)


def _hermite(p0, p1, m0, m1, u, dt):
    u2, u3 = u * u, u * u * u
    h00 = 2 * u3 - 3 * u2 + 1
    h10 = u3 - 2 * u2 + u
    h01 = -2 * u3 + 3 * u2
    h11 = u3 - u2
    return tuple(h00 * p0[i] + h10 * dt * m0[i] + h01 * p1[i] + h11 * dt * m1[i] for i in range(3))


def _tangent(k, field):
    i = KEYS.index(k)
    if k.get("still"):
        return (0.0, 0.0, 0.0)
    if "v" in k and field == "pos":
        return k["v"]
    if i == 0 or i == len(KEYS) - 1:
        return (0.0, 0.0, 0.0)
    a, b = KEYS[i - 1], KEYS[i + 1]
    # Catmull-Rom, damped so long moves don't overshoot
    return tuple(0.8 * (b[field][c] - a[field][c]) / (b["t"] - a["t"]) for c in range(3))


def _norm(v):
    m = math.sqrt(sum(c * c for c in v))
    return tuple(c / m for c in v)


def _sample(t, field):
    if t <= KEYS[0]["t"]:
        return KEYS[0][field]
    for a, b in zip(KEYS, KEYS[1:]):
        if t <= b["t"] and field == "tgt" and ("dir" in a or "dir" in b):
            # look direction interpolated between keys (for moves that turn a lot)
            u = smoother((t - a["t"]) / (b["t"] - a["t"]))
            pa = _sample(a["t"], "pos")
            da = a["dir"] if "dir" in a else tuple(a["tgt"][i] - pa[i] for i in range(3))
            pb = _sample(b["t"], "pos")
            db = b["dir"] if "dir" in b else tuple(b["tgt"][i] - pb[i] for i in range(3))
            d = _norm(lerp3(_norm(da), _norm(db), u))
            p = _sample(t, "pos")
            return tuple(p[i] + d[i] * 3 for i in range(3))
        if t <= b["t"]:
            dt = b["t"] - a["t"]
            u = (t - a["t"]) / dt
            if field in ("pos", "tgt"):
                return _hermite(a[field], b[field], _tangent(a, field), _tangent(b, field), u, dt)
            return lerp(a[field], b[field], smooth(u))
    if field == "tgt" and "dir" in KEYS[-1]:
        p = KEYS[-1]["pos"]
        return tuple(p[i] + KEYS[-1]["dir"][i] for i in range(3))
    return KEYS[-1][field]


def cam_pos(t):
    return _sample(t, "pos")


def cam_tgt(t):
    return _sample(t, "tgt")


def cam_focal(t):
    return _sample(t, "f")


def cam_focus(t):
    return _sample(t, "fd")


# --- Request pane (shots 1-3) ------------------------------------------------------

def request_pose(t):
    """(pos, yaw_deg, visible). yaw 0 faces +Z (front: the chat bubble); 180 shows the spec."""
    rx, ry, rz = REQ_POS
    flip = 180 * spring(t - T["flip"], zeta=0.55, w=8.0)
    bob = 0.015 * math.sin(t * 1.3)
    pos = (rx, ry + bob, rz)
    # shot 3: glide down and across into the machine's -X slit, edge first
    a = smoother(seg(t, 11.0, T["enter0"]))
    pos = lerp3(pos, (0.0, MACHINE["y1"] + PANE_H / 2 + 0.15, 0.0), a)
    b = ease_in(seg(t, T["enter0"], T["enter1"]), 2)
    pos = lerp3(pos, (0.0, PANE_Y, 0.0), b)
    yaw = flip                    # 180: the spec side faces the camera
    return pos, yaw, t < T["enter1"] + 0.05


# --- Captions in the world -----------------------------------------------------------
# Each: text, anchor (x, y, z), height in metres, kind, t_in, t_out.

CAPTIONS = [
    dict(text="An agent writes the plan.", at=(REQ_POS[0] + 1.0, 0.0, 0.7), h=0.2, t0=7.0, t1=12.5, kind="stand"),
    dict(text="Anikit Video turns it into frames.", at=(-1.0, 0.0, 1.3), h=0.14, t0=16.5, t1=21.0, kind="stand"),
    dict(text="3,600 frames · 60 s of 1080p at 60 fps", at=None, h=0.06, t0=18.0, t1=31.0, kind="counter"),
    dict(text="Anikit Video: all 3,600 frames in 3.4 s", at=(browser_x(34.0) - 10.0, 0.0, 1.3), h=0.45, t0=30.0, t1=40.5, kind="stand"),
    dict(text="Fastest browser setup: 69.1 s", at=(0.0, 1.9, BROWSER_Z), h=0.3, t0=30.0, t1=40.5, kind="cart"),
    dict(text="No browser. 21× faster.", at=(browser_x(39.0) + 12.0, 0.0, 1.3), h=0.6, t0=36.0, t1=41.0, kind="stand"),
    dict(text="sundial-launch.mp4 · H.264 · 1920×1080 · 60 fps", at=None, h=0.02, t0=56.0, t1=60.0, kind="etched"),
    dict(text="Its own H.264 encoder. One file.", at=(None, 0.0, 1.4), h=0.12, t0=54.0, t1=58.5, kind="stand"),
    dict(text="Still rendering.", at=None, h=0.5, t0=61.2, t1=64.5, kind="cart2"),
    dict(text="This film was one spec. Rendered in N s.", at=(None, 0.0, 10.0), h=0.45, t0=64.4, t1=68.5, kind="stand", yaw=180),
    dict(text="Anikit Video", at=None, h=0.6, t0=T["logo0"], t1=DURATION, kind="logo"),
    dict(text="Words in. Film out.", at=None, h=0.12, t0=68.0, t1=DURATION, kind="stand"),
]


def caption_anchor(c):
    xb = fold_centre()
    if c["kind"] == "etched":
        return (xb, 0.03, 0.0)
    if c["kind"] == "logo":
        return LOGO_POS
    if c["text"].startswith("Words in"):
        return (LOGO_POS[0], 0.0, LOGO_Z - 0.9)
    x, y, z = c["at"]
    return (xb if x is None else x, y, z)
