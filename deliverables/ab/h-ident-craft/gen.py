"""Kestrel ident — "THE GATE". Generates spec.json.

A hand-inked kestrel lives on a strip of 35 mm film. The film races through a
projector gate while the bird holds dead still (the hover = the held frame).
The film stops; the bird folds; the pull-down drops it out of the gate (the
stoop) and the next frame registers: KESTREL.
"""
import json, math, os, random, re
import kestrel as K

HERE = os.path.dirname(os.path.abspath(__file__))
W, H, FPS = 1920, 1080, 24

# ---- world rules: palette ---------------------------------------------------
INK = "#0E0906"
CLEAR = "#FFF4E0"        # clear film: transmits the lamp
BASE = "#7A3D17"         # acetate base, chestnut (= the kestrel's back)
PERF = "#FFE6BF"         # perforation holes: lamp seen straight through
EDGE = "#F2B266"         # edge print

# ---- strip geometry -----------------------------------------------------------
GX, GY = 960, 540        # gate (image centre)
FW, FH = 560, 408        # 1.37 academy frame
P = 440                  # frame pitch
KMIN, KMAX = -11, 4

# ---- timing ---------------------------------------------------------------------
T_STOP = 2.80            # film registers and stops
T_PULL = 3.20            # pull-down (the stoop) starts
T_HIT = 3.50             # title registers  (= beat 8 at 120 bpm)
T_CUE = 5.50             # changeover cue mark (4 frames)
DUR = 6.0
V, TD = 1500.0, 1.1      # run speed px/s, deceleration time

PIN = "M0 0 L0.01 0 L0 0.01 Z M1920 1080 L1919.99 1080 L1920 1079.99 Z "

def off(d, dx, dy, s=1.0):
    out, xy = [], 0
    for t in re.findall(r"[MCLQZ]|-?\d+\.?\d*", d):
        if t.isalpha():
            out.append(t); xy = 0
        else:
            v = float(t) * s + (dx if xy % 2 == 0 else dy); xy += 1
            out.append(f"{v:.1f}")
    return " ".join(out)

def path(d, **kw):
    nums = [float(v) for v in re.findall(r"-?\d+\.?\d*", d)]
    mx, my = math.floor(min(nums[0::2])) - 4, math.floor(min(nums[1::2])) - 4
    pin = f"M{mx} {my} L{mx+0.01} {my} L{mx} {my+0.01} Z "
    L = {"shape": "path", "d": pin + d, "x": mx, "y": my, "anchor": "topLeft"}
    L.update(kw)
    return L

def scroll(t):
    """strip offset (px) at time t during the run; 0 at the stop."""
    tau = T_STOP - t
    if tau <= 0:
        return 0.0
    if tau > TD:
        return -(V * (tau - TD) + V * TD / 2)
    return -(V * tau * tau / (2 * TD))

SCROLL_EXPR = (f"=540 - (gt<{T_STOP} ? (({T_STOP}-gt)>{TD} ? {V}*(({T_STOP}-gt)-{TD})+{V*TD/2} : "
               f"{V}*({T_STOP}-gt)*({T_STOP}-gt)/{2*TD}) : 0)"
               f" + (gt>={T_STOP} && gt<{T_STOP+0.3} ? 7*exp(-(gt-{T_STOP})*22)*sin((gt-{T_STOP})*70) : 0)")

def bird_layers(pose, cx, cy, seed, jitter=0.9, eye=True, extra=None):
    b = K.bird(pose, jitter=jitter, seed=seed)
    Ls = [path(off(b[k], cx, cy), fill=INK) for k in ("far", "tail", "body", "near")]
    Ls.append(path(off(b["scratch"], cx, cy), fill="none", stroke=CLEAR, strokeWidth=1.4))
    if eye:
        Ls.append({"shape": "circle", "width": 4.5, "height": 4.5, "fill": CLEAR, "x": cx, "y": cy})
    return Ls

BIRD_DY = -62            # eye position inside the frame, relative to the frame centre

def rough_rect(cx, cy, w, h, seed, j=1.6, n=9):
    rnd = random.Random(seed)
    pts = []
    for i in range(n):
        pts.append((cx - w / 2 + w * i / n, cy - h / 2))
    for i in range(n):
        pts.append((cx + w / 2, cy - h / 2 + h * i / n))
    for i in range(n):
        pts.append((cx + w / 2 - w * i / n, cy + h / 2))
    for i in range(n):
        pts.append((cx - w / 2, cy + h / 2 - h * i / n))
    pts = [(x + rnd.uniform(-j, j), y + rnd.uniform(-j, j)) for x, y in pts]
    return "M" + " L".join(f"{x:.1f} {y:.1f}" for x, y in pts) + " Z"

def soundtrack(cy, seed):
    """variable-area optical track for one frame: a clear band whose width wiggles."""
    rnd = random.Random(seed)
    x0, x1 = GX - 376, GX - 314
    mid = (x0 + x1) / 2
    ys = [cy - P / 2 + i * 8 for i in range(int(P / 8) + 1)]
    ph = rnd.uniform(0, 6)
    amp = [abs(math.sin(y * 0.045 + ph) * 0.6 + math.sin(y * 0.19 + ph * 2) * 0.3 + rnd.uniform(-0.12, 0.12)) for y in ys]
    right = [(mid + 3 + 26 * a, y) for a, y in zip(amp, ys)]
    left = [(mid - 3 - 26 * a, y) for a, y in zip(amp, ys)][::-1]
    pts = right + left
    return "M" + " L".join(f"{x:.1f} {y:.1f}" for x, y in pts) + " Z"

# ---- the strip ----------------------------------------------------------------------
def strip_layers():
    Ls = []
    top = GY - KMAX * P - P / 2
    bot = GY - KMIN * P + P / 2
    Ls.append({"shape": "rect", "x": GX - 470, "y": top, "anchor": "topLeft", "width": 870, "height": bot - top, "fill": BASE})
    for k in range(KMIN, KMAX + 1):
        cy = GY - k * P
        # perforations: 4 per frame, both sides
        for j in range(4):
            py = cy - P / 2 + P / 8 + j * P / 4
            for px in (GX - 418, GX + 326):
                Ls.append({"shape": "rect", "x": px, "y": py, "width": 40, "height": 30, "radius": 6, "fill": PERF})
        Ls.append(path(soundtrack(cy, 100 + k), fill="#F6D6A6", opacity=0.9))
        # edge print, only legible when the film is still
        Ls.append({"text": f"KESTREL  5219  {abs(k)+24:02d}", "font": "JetBrains Mono", "size": 18, "weight": 500,
                   "letterSpacing": "0.18em", "color": EDGE, "x": GX + 374, "y": cy, "rotation": 90})
        Ls.append({"text": "▸", "font": "JetBrains Mono", "size": 14, "color": EDGE, "x": GX - 452, "y": cy - P / 2 + 30})
        # the frame
        if k == 1:
            Ls += title_frame(cy)
            continue
        Ls.append(path(rough_rect(GX, cy, FW, FH, 300 + k, j=1.2), fill=CLEAR))
        if k <= -1:
            pose = K.HOVER[(k % 6)]
            Ls += bird_layers(pose, GX, cy + BIRD_DY, seed=500 + k)
        elif k == 0:
            Ls += bird_layers(K.FOLD[2], GX, cy + BIRD_DY, seed=777)
        # k >= 2: clear leader — the film to come
    return Ls

def title_frame(cy):
    Ls = [path(rough_rect(GX, cy, FW, FH, 901, j=1.2), fill=CLEAR),
          path(rough_rect(GX, cy, FW - 14, FH - 14, 902, j=2.2), fill=INK)]
    # the mark: the kestrel in its stoop, scratched clear out of the ink
    s = K.stoop(seed=3, jitter=0.5)
    for k in ("wl", "wr", "tail", "body"):
        Ls.append(path(off(s[k], GX, cy - 22, 0.72), fill=CLEAR))
    Ls.append({"text": "KESTREL", "font": "Inter", "size": 58, "weight": 560, "letterSpacing": "0.46em",
               "color": CLEAR, "x": GX + 13, "y": cy + 42,
               "animate": [{"keyframes": {"x": [GX + 13, GX + 13.6, GX + 12.5, GX + 13]}, "at": 0, "duration": 0.25, "repeat": -1, "ease": "steps(4)"}]})
    # changeover cue mark, top-right, 4 frames
    Ls.append({"shape": "circle", "width": 26, "height": 26, "fill": CLEAR, "x": GX + 236, "y": cy - 164,
               "opacity": f"=(gt>={T_CUE} && gt<{T_CUE + 4/24 - 0.001}) ? 1 : 0"})
    return Ls

# ---- the hero in the gate while the film runs --------------------------------------
def hero_layers():
    Ls = [path(rough_rect(GX, GY, FW, FH, 300, j=1.2), fill=CLEAR)]
    # 6 hover poses x 2 ink variants, one per film frame (4 wingbeats a second)
    for v in range(2):
        for j, pose in enumerate(K.HOVER):
            idx = v * 6 + j
            g = {"layers": bird_layers(pose, GX, GY + BIRD_DY, seed=10 + idx),
                 "opacity": f"=(gt<2.62 && floor(gt*24) % 12 == {idx}) ? 1 : 0"}
            Ls.append(g)
    wins = [(2.62, 2.70), (2.70, 2.78), (2.78, 9)]
    for f, (a, b) in enumerate(wins):
        Ls.append({"layers": bird_layers(K.FOLD[f], GX, GY + BIRD_DY, seed=777 if f == 2 else 60 + f),
                   "opacity": f"=(gt>={a} && gt<{b}) ? 1 : 0"})
    return {"layers": Ls, "end": T_STOP}

def dust_layers():
    Ls = []
    for i in range(6):
        Ls.append({"shape": "circle", "width": 2.5 + (i % 3) * 1.6, "height": 2 + (i % 2) * 2.2,
                   "fill": INK if i % 2 == 0 else CLEAR,
                   "x": f"={GX} + (rand(frame*7+{i*31})-0.5)*{FW-30}",
                   "y": f"={GY} + (rand(frame*13+{i*17})-0.5)*{FH-30}",
                   "opacity": f"=rand(frame*3+{i*5}) > 0.55 ? 0.9 : 0"})
    # a hair that drifts through for a few frames
    Ls.append(path(f"M{GX+150} {GY-120} Q{GX+164} {GY-90} {GX+154} {GY-60} Q{GX+144} {GY-34} {GX+162} {GY-12}", fill="none", stroke=INK, strokeWidth=1.3,
                   opacity="=(gt>1.1 && gt<1.4) || (gt>4.3 && gt<4.42) ? 0.8 : 0"))
    return Ls

def light_layers():
    """the lamp: one light field, multiplied over everything on the film."""
    return {"blend": "multiply", "layers": [
        {"shape": "rect", "width": 6000, "height": 5000, "fill": "#24170E"},
        {"shape": "circle", "width": 1900, "height": 1500, "x": GX, "y": GY + 140,
         "fill": "radial-gradient(#9C6A3C, #24170E)", "opacity": 0.95},
        {"shape": "rect", "width": FW + 26, "height": FH + 26, "x": GX, "y": GY, "radius": 10, "blur": 16,
         "fill": "#FFFFFF", "opacity": "=0.94 + 0.06*rand(frame)"},
    ]}

def tick_times():
    """one tick per perforation passing the sprocket (sound made by the picture)."""
    ts, last = [], math.floor(scroll(0) / (P / 4))
    t = 0.0
    while t < T_STOP:
        n = math.floor(scroll(t) / (P / 4))
        if n != last:
            ts.append(round(t, 4)); last = n
        t += 1 / 2000
    return ts

def build():
    weave = {"x": "=960 + 0.9*noise(gt*6)", "y": "=540 + 0.7*noise(gt*6+11)",
             "layers": [
                 {"id": "pull", "animate": [
                     {"to": {"y": 540 + P}, "at": T_PULL, "duration": T_HIT - T_PULL, "ease": "inCubic"},
                     {"keyframes": {"y": [540 + P, 540 + P - 9, 540 + P + 3, 540 + P]}, "at": T_HIT, "duration": 0.16, "ease": "outQuad"}],
                  "layers": [{"id": "strip", "y": SCROLL_EXPR, "layers": strip_layers()}]},
                 hero_layers(),
             ] + dust_layers()}
    layers = [weave, light_layers()]

    audio = []
    for i, t in enumerate(tick_times()):
        audio.append({"sfx": "tick", "at": t, "volume": "-9dB", "pitch": 0.9 + 0.12 * (i % 3)})
    # the claw: four perforations pulled down during the stoop
    for j, t in enumerate([3.29, 3.37, 3.43, 3.475]):
        audio.append({"sfx": "tick", "at": t, "volume": "-12dB", "pitch": 0.8})
    audio += [
        {"sfx": "whoosh", "at": 3.22, "volume": "-9dB", "duration": 0.3},
        {"sfx": "impact", "at": T_HIT, "volume": "-10dB"},
        {"sfx": "click", "at": T_CUE, "volume": "-10dB"},
    ]
    # button: the next projector spins up as the feature begins
    for j, t in enumerate([5.66, 5.80, 5.90, 5.97]):
        audio.append({"sfx": "tick", "at": t, "volume": f"-{24 - j * 2}dB", "pitch": 0.9})

    music = {
        "bpm": 120, "key": "D", "length": f"{DUR}s", "end": "ring", "followCuts": False,
        "master": {"lufs": -15, "ceiling": -2.5},
        "define": {"wind": {"synth": {"osc": [{"wave": "sine", "level": 0.0}], "noise": 1.0,
                                      "filter": {"type": "bp", "cutoff": 650, "res": 0.25},
                                      "amp": [0.25, 0.1, 1, 0.04], "lfo": {"rate": 0.7, "cutoff": 0.5}}}},
        "tracks": [
            {"inst": "wind", "notes": "C4:5.6", "gain": -14},
            {"inst": "cello", "notes": "o2 D:5.6", "gain": -7},
            {"inst": "strings", "notes": "r:7 [D3 A3 E4 F#4]:6", "gain": -5, "reverb": 0.45},
            {"inst": "piano", "notes": "r:7 [D1 D2]:6", "gain": -3},
            {"inst": "flute", "notes": "r:11 E6/16^ E6/16^ E6/16^", "gain": -9, "reverb": 0.3},
        ],
    }

    spec = {
        "size": "16:9", "fps": FPS, "background": "#050302",
        "intent": {"pace": "steady", "genre": "brand-film"},
        "motionBlur": {"shutter": 180},
        "post": {"bloom": 0.25, "bloomThreshold": 0.82, "halation": 0.45, "vignette": 0.42,
                 "grain": 0.045, "grainSize": 1.4, "contrast": 1.06, "temperature": 0.05},
        "music": music,
        "audio": audio,
        "scenes": [{
            "duration": DUR,
            "camera": {"zoom": 0.94, "y": 125, "animate": [
                {"to": {"zoom": 1.13}, "at": 0, "duration": T_STOP, "ease": "inOutSine"},
                {"to": {"zoom": 1.28, "y": 0}, "at": T_HIT, "duration": DUR - T_HIT, "ease": "outCubic"}],
                "shake": {"amount": 7, "frequency": 14, "at": T_HIT, "duration": 0.35}},
            "layers": layers,
        }],
    }
    return spec

if __name__ == "__main__":
    spec = build()
    with open(os.path.join(HERE, "spec.json"), "w") as f:
        json.dump(spec, f, indent=1)
    print("ticks:", len(tick_times()))
