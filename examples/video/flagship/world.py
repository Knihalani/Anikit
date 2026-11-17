"""EVERY FRAME: the world, as functions of time.

Single source of truth for the film's layout, timing and camera. `previz.py`
draws a grey-box animatic from it; `build.py` turns it into the anikit-video
spec; `music.py` takes its sync points from it. Units: metres and seconds.
Y up, floor at y = 0. Strips run along +X; panes face +Z; lane k sits at
z = -4k, so the camera, which lives on the +Z side of a lane, hops towards -Z.
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

# --- The six films ------------------------------------------------------------
FILMS = [
    dict(id="product", name="Halo", file="halo.mp4",
         request="A launch film for Halo, our speaker. Polished steel and glass, slow and expensive."),
    dict(id="fold", name="Fold", file="fold.mp4",
         request="Something abstract: a glass sculpture folding through itself, slow and iridescent."),
    dict(id="tide", name="Tide", file="tide.mp4",
         request="Sunlight through moving water, for a swim brand. No words, just light."),
    dict(id="type", name="No Sleep", file="no-sleep.mp4",
         request="A lyric video for 'No Sleep', 128 BPM. Huge 3D type, acid yellow, in the dark."),
]
LANES = len(FILMS)
LANE_DZ = -4.0
FILM_SECONDS = 30.0
FRAMES = int(FILM_SECONDS * FPS)      # frames per film = panes per strip


def lane_z(k):
    return k * LANE_DZ


# --- Objects ---------------------------------------------------------------
PANE_W, PANE_H, PANE_T = 1.6, 0.9, 0.008
PANE_Y = PANE_H / 2 + 0.02          # panes stand just above the floor
PITCH = 1.7                          # pane centre spacing along a strip
LOCK_SPEED = PITCH * FPS             # m/s: one pane per output frame = the film plays

# The gantry: 2 m deep in X, spans all six lanes in Z.
MACHINE = dict(x0=-1.0, x1=1.0, y0=0.0, y1=1.1, z0=lane_z(LANES - 1) - 0.6, z1=0.6)

# --- Timeline (seconds) ------------------------------------------------------
T = dict(
    reveal0=3.5, reveal1=7.0,     # pull back from pane 0 to all six requests
    flip=7.0, flip_step=0.12,     # flips ripple from lane 0 to lane 5
    move0=10.0,                   # requests glide to above their slots
    drop0=12.5,                   # first drop lands; one per beat -> 15.0
    led=15.0,
    frame1_0=15.5, frame1_1=16.5,
    stream0=18.0, stream1=25.5,   # 2 .. FRAMES; the machine leaves over the horizon
    run0=26.0, run1=50.0,         # the locked run; lane k from run0 + 4k
    hop=0.8,                      # each hop lands on the downbeat
    brake1=50.8,
    fold0=50.6, fold1=52.0,       # strips fold into stacks (land on 52.0)
    crush0=52.5, crush1=54.0,     # stacks crush into blocks
    shrink0=54.5, shrink1=56.0,   # blocks shrink into tiles
    rise0=57.2, rise1=59.2,       # tiles rise, turn and grow into screens
    part0=66.0, part1=67.2,       # screens part for the logo
    logo0=66.2, logo1=68.0,
)
RUN_LANE_SECONDS = (T["run1"] - T["run0"]) / LANES


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


# --- Requests (shots 1-3) --------------------------------------------------------

def request_home(k):
    """Where request pane k hangs in shots 1-2: a rising diagonal behind pane 0."""
    return (-5.0 + 1.5 * k, 1.05 + 0.12 * k, lane_z(k) * 0.9)


def drop_time(k):
    return T["drop0"] + k * BEAT


def request_pose(k, t):
    """(pos, yaw_deg, visible) of request pane k. yaw 0: the request faces +Z; 180: the plan does."""
    hx, hy, hz = request_home(k)
    flip = 180 * spring(t - (T["flip"] + k * T["flip_step"]), zeta=0.55, w=8.0)
    bob = 0.015 * math.sin(t * 1.3 + k)
    pos = (hx, hy + bob, hz)
    a = smoother(seg(t, T["move0"] + 0.15 * k, drop_time(k) - 0.45))
    pos = lerp3(pos, (0.0, MACHINE["y1"] + PANE_H / 2 + 0.2, lane_z(k)), a)
    b = ease_in(seg(t, drop_time(k) - 0.45, drop_time(k)), 2)
    pos = lerp3(pos, (0.0, PANE_Y, lane_z(k)), b)
    return pos, flip, t < drop_time(k) + 0.05


# --- Frame emission ----------------------------------------------------------

def emitted(t: float) -> float:
    """Continuous count of frames laid per strip (n). Frame j is fully revealed at n = j + 1."""
    if t < T["frame1_0"]:
        return 0.0
    if t < T["stream0"]:
        return smoother(seg(t, T["frame1_0"], T["frame1_1"]))
    s0 = T["stream0"]
    if t < s0 + 1.5:                       # frames 2, 3, 4 on the beat
        k = (t - s0) / BEAT
        i = math.floor(k)
        return 1 + i + smoother((k - i) / 0.6)
    u = seg(t, s0 + 1.5, T["stream1"])
    if u >= 1:
        return float(FRAMES)
    return 4 + (FRAMES - 4) * (u ** 2.6)


def pane_x(j: int) -> float:
    """Frame j stands still where the machine left it (frame 0 where the plan went in)."""
    return j * PITCH - 0.15


def machine_x(n: float) -> float:
    return n * PITCH


def frame_at_x(x: float) -> float:
    return (x + 0.15) / PITCH


# --- The run (shot 5) ------------------------------------------------------------

CAM_Z = 3.6                    # camera offset (z) from the strip during the run
CAM_LOCK_Y = 0.72
HOP_Y = 1.9
RUN_LOOK = (5.0, -0.42, -CAM_Z)   # oblique: looking ~36 degrees down the strip, slightly down
RUN_FOCAL = 50.0
RUN_X0 = pane_x(int(2.0 * FPS))     # lock starts at film time 2 s
RUN_LEN = LOCK_SPEED * (T["run1"] - T["run0"])


def run_lane(t):
    return int(clamp((t - T["run0"]) // RUN_LANE_SECONDS, 0, LANES - 1))


def run_pos(t):
    """Camera position during the run (exactly linear in x; hops in y/z land on downbeats)."""
    x = RUN_X0 + LOCK_SPEED * (t - T["run0"])
    k = run_lane(t)
    z = lane_z(k) + CAM_Z
    y = CAM_LOCK_Y
    t_land = T["run0"] + RUN_LANE_SECONDS * (k + 1)       # the next hop lands here
    if k < LANES - 1 and t > t_land - T["hop"]:
        u = seg(t, t_land - T["hop"], t_land)
        z = lerp(z, lane_z(k + 1) + CAM_Z, smoother(u))
        y = CAM_LOCK_Y + (HOP_Y - CAM_LOCK_Y) * math.sin(math.pi * smooth(u))
    return (x, y, z)


def run_dir(t):
    """Look direction during the run: at the strip, tipping down over the top of each hop."""
    k = run_lane(t)
    t_land = T["run0"] + RUN_LANE_SECONDS * (k + 1)
    dy = RUN_LOOK[1]
    if k < LANES - 1 and t > t_land - T["hop"]:
        u = seg(t, t_land - T["hop"], t_land)
        dy = RUN_LOOK[1] - 1.6 * math.sin(math.pi * u)
    return (RUN_LOOK[0], dy, RUN_LOOK[2])


# --- Fold (shot 6) -------------------------------------------------------------
STACK_STEP = 0.012


def fold_centre():
    return KEYS_BRAKE_X


def fold_jc():
    return int(round(frame_at_x(fold_centre())))


def pane_pose_fold(j: int, t: float):
    """(x, yaw_deg) of pane j of any strip during the fold and crush."""
    xc, jc = fold_centre(), fold_jc()
    x0 = pane_x(j)
    xs = xc - (j - jc) * STACK_STEP
    d = abs(j - jc)
    delay = 0.3 * (1 - math.exp(-d / 300))
    u = smoother(seg(t, T["fold0"] + delay, T["fold1"]))
    x = lerp(x0, xs, u)
    yaw = 90 * smoother(seg(t, T["fold0"] + delay * 0.5, T["fold0"] + delay + 0.5))
    c = smoother(seg(t, T["crush0"], T["crush1"]))
    x = xc + (x - xc) * lerp(1.0, 0.25 / (FRAMES * STACK_STEP), c)
    return x, yaw


def block_scale(t):
    """Scale of each crushed block as it shrinks into its tile (1 -> tile)."""
    return lerp(1.0, 0.12, smoother(seg(t, T["shrink0"], T["shrink1"])))


TILE_Z0 = -2.0
TILE_GAP = 0.26


def block_z(k, t):
    """Lane k's block slides into a tight row of six tiles while it shrinks."""
    return lerp(lane_z(k), TILE_Z0 + (k - (LANES - 1) / 2) * TILE_GAP, smoother(seg(t, T["shrink0"], T["shrink1"])))


# --- Screens (shot 7) ----------------------------------------------------------------
SCREEN_Z = -6.0
SCREEN_Y = 1.25
SCREEN_GAP = 2.05


def screen_pose(k, t):
    """(pos, yaw, scale) of film k's tile/screen from shrink1 to the end. scale 1 = a 1.6 m screen."""
    xc = fold_centre()
    tile = (xc, 0.06, block_z(k, T["shrink1"]))
    home = (xc + (k - (LANES - 1) / 2) * SCREEN_GAP, SCREEN_Y, SCREEN_Z)
    st = T["rise0"] + 0.15 * k
    u = smoother(seg(t, st, st + 1.6))
    pos = lerp3(tile, home, u)
    pos = (pos[0], pos[1] + 0.6 * math.sin(math.pi * u), pos[2])
    yaw = lerp(90.0, 0.0, smoother(seg(t, st + 0.2, st + 1.4)))
    scale = lerp(0.12, 1.0, smoother(seg(t, st + 0.3, st + 1.6)))
    p = smoother(seg(t, T["part0"], T["part1"]))
    side = k - (LANES - 1) / 2
    pos = (pos[0] + p * side * 1.6, pos[1] + p * (2.6 - 0.25 * abs(side)), pos[2] + p * 0.5)
    return pos, yaw, scale


# --- Camera --------------------------------------------------------------------
# Keys: (t, pos, tgt | dir, focal_mm, focus_dist). Cubic Hermite with Catmull-Rom
# tangents unless a velocity is given; `still` keys have zero tangents. The run
# (run0..run1) is not keyed: it is run_pos/run_dir exactly.

LOGO_Z = SCREEN_Z - 5.0
LOGO_Z_DIST = (SCREEN_Z + 2.5) - LOGO_Z


def _keys():
    x_brake = RUN_X0 + RUN_LEN + LOCK_SPEED * (T["brake1"] - T["run1"]) * 0.5
    globals()["KEYS_BRAKE_X"] = x_brake
    zl = lane_z(LANES - 1) + CAM_Z
    V = LOCK_SPEED
    r0 = request_home(0)
    mid_z = lane_z(LANES - 1) / 2
    return [
        # shot 1: push in on request 0, then pull back and rise to reveal all six
        dict(t=0.0, pos=(r0[0] + 0.3, r0[1] + 0.08, 4.6), tgt=r0, f=70, fd=4.6),
        dict(t=3.5, pos=(r0[0] + 0.05, r0[1], 3.5), tgt=r0, f=70, fd=3.5, still=True),
        dict(t=7.0, pos=(-10.5, 3.4, 9.5), tgt=(-1.5, 1.0, -8.5), f=40, fd=12.0),
        # shot 2: orbit while the plans appear
        dict(t=11.0, pos=(-9.0, 5.0, 11.0), tgt=(0.0, 0.8, -9.0), f=36, fd=14.0),
        # shot 3: descend to a three-quarter view; the six first frames stand in rows
        dict(t=15.0, pos=(-6.0, 5.5, 11.0), tgt=(0.5, 0.6, -8.0), f=34, fd=14.0),
        dict(t=17.5, pos=(6.0, 5.0, 10.0), tgt=(0.0, 0.5, -9.0), f=34, fd=14.0),
        # shot 4: crane up and back over the highway of strips, then dive forward to lane 0
        dict(t=21.5, pos=(-30.0, 12.0, 14.0), tgt=(60.0, 0.0, mid_z), f=30, fd=60, v=(25.0, 0.0, 0.0)),
        dict(t=T["run0"], pos=run_pos(T["run0"]), dir=run_dir(T["run0"]), f=RUN_FOCAL, fd=4.4, v=(V, 0, 0)),
        dict(t=T["run1"], pos=run_pos(T["run1"]), dir=run_dir(T["run1"]), f=RUN_FOCAL, fd=4.4, v=(V, 0, 0)),
        dict(t=T["brake1"], pos=(x_brake, CAM_LOCK_Y + 0.15, zl + 0.6), dir=(0.0, -0.1, -CAM_Z), f=35, fd=3.0,
             v=(0, 0, 0)),
        # shot 6: rise as the stacks arrive and crush, then arc along the six tiles
        dict(t=53.5, pos=(x_brake + 6.0, 4.0, zl + 4.0), tgt=(x_brake, 0.4, mid_z), f=35, fd=12.0),
        dict(t=56.8, pos=(x_brake + 0.85, 0.3, TILE_Z0 + 0.75), tgt=(x_brake, 0.05, TILE_Z0 - 0.1), f=50, fd=1.15, still=True),
        # shot 7: follow the tiles up as they turn into screens, glide along the row, pull back through it
        dict(t=58.8, pos=(x_brake + 1.2, 1.1, TILE_Z0 + 4.0), tgt=(x_brake, 1.15, SCREEN_Z), f=30, fd=7.0),
        dict(t=60.5, pos=(x_brake - 4.0, 1.4, SCREEN_Z + 4.5), tgt=(x_brake - 2.0, SCREEN_Y, SCREEN_Z), f=40,
             fd=4.8),
        dict(t=64.5, pos=(x_brake + 3.5, 1.5, SCREEN_Z + 5.0), tgt=(x_brake + 1.5, SCREEN_Y, SCREEN_Z), f=40,
             fd=5.2),
        # shot 8: push through the parted screens to the logo
        dict(t=70.0, pos=(x_brake, 1.4, SCREEN_Z + 2.5), tgt=(x_brake, 0.65, LOGO_Z), f=40, fd=LOGO_Z_DIST),
        dict(t=72.0, pos=(x_brake, 1.38, SCREEN_Z + 2.1), tgt=(x_brake, 0.65, LOGO_Z), f=40, fd=LOGO_Z_DIST - 0.4),
    ]


KEYS = _keys()
for _k in KEYS:
    if "dir" in _k:
        _m = math.sqrt(sum(c * c for c in _k["dir"]))
        _k["tgt"] = tuple(_k["pos"][i] + 3 * _k["dir"][i] / _m for i in range(3))
LOGO_POS = (KEYS_BRAKE_X, 0.0, LOGO_Z)


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
    return tuple(0.8 * (b[field][c] - a[field][c]) / (b["t"] - a["t"]) for c in range(3))


def _norm(v):
    m = math.sqrt(sum(c * c for c in v))
    return tuple(c / m for c in v)


def _sample(t, field):
    if T["run0"] <= t <= T["run1"]:
        if field == "pos":
            return run_pos(t)
        if field == "tgt":
            p, d = run_pos(t), _norm(run_dir(t))
            return tuple(p[i] + 3 * d[i] for i in range(3))
    if t <= KEYS[0]["t"]:
        return KEYS[0][field]
    for a, b in zip(KEYS, KEYS[1:]):
        if t <= b["t"] and field == "tgt" and ("dir" in a or "dir" in b):
            u = smoother((t - a["t"]) / (b["t"] - a["t"]))
            pa, pb = a["pos"], b["pos"]
            da = a["dir"] if "dir" in a else tuple(a["tgt"][i] - pa[i] for i in range(3))
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
    return KEYS[-1][field]


def cam_pos(t):
    return _sample(t, "pos")


def cam_tgt(t):
    return _sample(t, "tgt")


def cam_focal(t):
    if T["run0"] <= t <= T["run1"]:
        return RUN_FOCAL
    return _sample(t, "f")


def cam_focus(t):
    if T["run0"] <= t <= T["run1"]:
        return math.hypot(RUN_LOOK[0], RUN_LOOK[2]) * 0.72
    return _sample(t, "fd")


def shutter(t):
    """Shutter angle in degrees: closes while locked (panes would smear by half a pitch), opens in hops."""
    if T["run0"] + 1.0 <= t <= T["run1"]:
        k = run_lane(t)
        t_land = T["run0"] + RUN_LANE_SECONDS * (k + 1)
        if k < LANES - 1 and t > t_land - T["hop"]:
            return 90.0 * math.sin(math.pi * seg(t, t_land - T["hop"], t_land))
        return 0.0
    return 180.0


# --- Captions in the world -----------------------------------------------------------
# Each: text, anchor (x, y, z), height in metres, kind, t_in, t_out, yaw (0 faces +Z).

CAPTIONS = [
    dict(text="An agent writes each plan.", at=(-6.5, 0.0, -3.0), h=0.45, t0=7.6, t1=12.6, kind="stand"),
    dict(text="Anikit Video turns each into frames.", at=(-3.4, 0.0, 2.4), h=0.42, t0=16.2, t1=20.0,
         kind="stand"),
    *[dict(text=f["file"], at=None, lane=k, h=0.02, t0=55.0, t1=58.4, kind="etched") for k, f in enumerate(FILMS)],
    dict(text="Anikit Video", at=None, h=0.6, t0=T["logo0"], t1=DURATION, kind="logo"),
    dict(text="Words in. Film out.", at=None, h=0.16, t0=67.6, t1=DURATION, kind="stand"),
]


def caption_anchor(c):
    xb = fold_centre()
    if c["kind"] == "etched":
        return (xb + 0.02, 0.06, block_z(c["lane"], T["shrink1"]))
    if c["kind"] == "logo":
        return LOGO_POS
    if c["text"].startswith("Words in"):
        return (LOGO_POS[0], 0.0, LOGO_Z + 0.9)
    return c["at"]
