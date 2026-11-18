"""Assembles spec.json for the Anikit Video launch film ("The darkroom").

Run from deliverables/launch/anikit:  python3 src/build_spec.py
The spec is plain JSON; this script only keeps the sheet geometry (shared by the
sheet layers and the develop filters that must track them) in one place.
"""
import json

PAPER = "#F2EEE6"
INK = "#1b1714"
SERIF = "Newsreader"
MONO = "IBM Plex Mono"
BAR = 2.5  # 96 bpm, 4/4


# ---------------------------------------------------------------- compositions
def sea_pic(w, h):
    k = w / 2160.0
    return {
        "size": [w, h], "duration": 30, "background": "#000",
        "layers": [
            {"type": "shader", "src": "src/sea.glsl",
             "uniforms": {"tt": "=t", "speed": 0.8, "sun": 0.92, "swell": 1.1, "chop": 1.2,
                          "color": [0.01, 0.05, 0.08], "sky": [0.28, 0.32, 0.45]}},
            # headland
            {"shape": "path", "d": f"M0 {120*k} C{140*k} {70*k} {260*k} {44*k} {380*k} {40*k} C{470*k} {36*k} {560*k} {60*k} {760*k} {96*k} L{760*k} {500*k} L0 {500*k} Z",
             "fill": "#07080a", "x": 1420 * k, "y": 610 * k, "anchor": "topLeft"},
            # tower and lantern
            {"shape": "path", "d": f"M{16*k} 0 L{54*k} 0 L{66*k} {340*k} L{4*k} {340*k} Z", "fill": "#0b0c0e",
             "x": 1700 * k, "y": 312 * k, "anchor": "topLeft"},
            {"shape": "rect", "width": 70 * k, "height": 50 * k, "fill": "#0b0c0e", "x": 1700 * k, "y": 266 * k, "anchor": "topLeft"},
            {"shape": "rect", "width": 50 * k, "height": 30 * k, "fill": "#fff3d6", "x": 1710 * k, "y": 279 * k,
             "anchor": "topLeft", "glow": "#ffd9a0"},
            # the beam, turning (seen side-on its length swings through the lantern)
            {"shape": "path", "d": f"M0 {150*k} L{2000*k} 0 L{2000*k} {340*k} Z",
             "fill": "linear-gradient(90deg, rgba(255,240,210,0.9), rgba(255,240,210,0))",
             "x": 1735 * k, "y": 294 * k, "anchor": "left", "blend": "screen", "scaleX": "=-cos(t*1.25 + 0.4)"},
            # the boat
            {"shape": "path", "d": f"M0 {18*k} L{90*k} {18*k} L{76*k} {34*k} L{10*k} {34*k} Z M{40*k} {18*k} L{40*k} {-34*k} L{72*k} {14*k} Z",
             "fill": "#050506", "x": f"={520*k} + {6*k}*sin(t*1.7)", "y": f"={760*k} + {5*k}*sin(t*2.3)",
             "rotation": "=3*sin(t*1.9)", "anchor": "topLeft"},
            # rain
            {"repeat": 90, "shape": "rect", "width": max(2, 3 * k), "height": 90 * k, "fill": "rgba(225,230,240,0.30)",
             "rotation": 12, "x": f"=($i*271) % {w}", "y": f"=(($i*157) + t*{1600*k}) % {h+140*k} - {70*k}", "anchor": "topLeft"},
        ]}


def glass_pic(w, h, quality):
    return {
        "size": [w, h], "duration": 30, "background": "#000",
        "layers": [{"component": "scene3d", "props": {
            "quality": quality,
            "env": {"preset": "interior", "color": "#141210"},
            "rig": {"preset": "window", "gobo": "window", "warmth": 0.45, "animate": "sweep", "duration": 9},
            "floor": {"color": "#3a2a1c", "roughness": 0.45, "reflect": 0.2},
            "camera": {"keys": [
                {"t": 0, "pos": [0.55, 0.62, 5.4], "target": [0.25, 0.3, 0], "focal": 62, "fstop": 3.2, "focus": "g2"},
                {"t": 6, "pos": [0.0, 0.58, 4.9], "target": [0.25, 0.3, 0], "focal": 62, "fstop": 3.2, "focus": "g2"}]},
            "objects": [
                {"type": "lathe", "name": "g1", "points": [[0, 0], [0.14, 0], [0.15, 0.02], [0.17, 0.5], [0.155, 0.5], [0.135, 0.03], [0, 0.03]],
                 "segments": 96, "pos": [-0.4, 0, 0.1], "material": "glass"},
                {"type": "lathe", "name": "g2", "points": [[0, 0], [0.16, 0], [0.17, 0.02], [0.18, 0.36], [0.165, 0.36], [0.15, 0.03], [0, 0.03]],
                 "segments": 96, "pos": [0.3, 0, -0.25], "material": "glass"},
                {"type": "lathe", "name": "g3", "points": [[0, 0], [0.11, 0], [0.12, 0.02], [0.13, 0.66], [0.118, 0.66], [0.105, 0.03], [0, 0.03]],
                 "segments": 96, "pos": [0.9, 0, 0.2], "material": "glass"},
                {"type": "sphere", "radius": 0.07, "pos": [0.05, 0.07, 0.5], "material": {"extends": "clay", "base": "#c98a3a"}},
            ]}}]}


def ink_pic(w, h):
    return {"size": [w, h], "duration": 30, "background": "#000",
            "layers": [{"type": "shader", "src": "src/inkdrop.glsl",
                        "uniforms": {"tt": "=t + 2.0", "speed": 0.8, "size": 1.1, "mixing": 0.45, "center": [0.0, 0.4]}}]}


def type_pic(w, h):
    k = w / 2160.0
    letters = ["R", "a", "i", "n", "."]
    xs = [0, 470, 830, 1010, 1450]
    layers = [{"shape": "rect", "width": w, "height": h, "fill": "#16130f", "x": 0, "y": 0, "anchor": "topLeft"}]
    for i, (ch, x) in enumerate(zip(letters, xs)):
        t0 = 0.25 + 0.22 * i
        layers.append({
            "text": ch, "font": SERIF, "weight": 600, "size": 760 * k, "color": "#EDE6DA",
            "x": (250 + x) * k, "y": 900 * k, "anchor": "bottomLeft",
            "animate": [{"from": {"y": -260 * k}, "to": {"y": 900 * k}, "at": t0, "duration": 0.55, "ease": "outBounce"}]})
    # drops that keep falling between the letters
    layers.append({"repeat": 40, "shape": "rect", "width": max(2, 3 * k), "height": 60 * k, "fill": "rgba(237,230,218,0.35)",
                   "x": f"=($i*331) % {w}", "y": f"=(($i*211) + t*{1300*k}) % {h+120*k} - {60*k}", "anchor": "topLeft"})
    return {"size": [w, h], "duration": 30, "background": "#16130f", "layers": layers}


def sheet(pic, caption, W):
    """A print on fibre paper: the picture with a white border and its caption (the agent's words)."""
    H = W * 2 / 3
    m = W * 0.05
    pw, ph = W - 2 * m, (W - 2 * m) * 1160 / 2160
    return {"size": [round(W), round(H)], "duration": 30, "background": PAPER,
            "layers": [
                {"composition": pic, "x": m, "y": m, "anchor": "topLeft", "width": pw, "height": ph},
                {"text": f"*{caption}*", "font": SERIF, "size": round(W * 0.0315), "color": INK,
                 "x": m, "y": m + ph + (H - m - ph - m) / 2 + m * 0.35, "anchor": "left"},
            ]}


def name_sheet(W):
    H = W * 2 / 3
    m = W * 0.05
    return {"size": [round(W), round(H)], "duration": 30, "background": PAPER,
            "layers": [
                {"text": "Anikit Video", "font": SERIF, "weight": 500, "size": round(W * 0.125), "color": INK,
                 "letterSpacing": "-0.02em", "x": m * 1.2, "y": H * 0.47, "anchor": "left"},
                {"text": "anikit-video render spec.json", "font": MONO, "size": round(W * 0.042), "color": INK,
                 "x": m * 1.3, "y": H * 0.74, "anchor": "left", "start": 1.7, "enter": "fade"},
            ]}


def name_pic(w, h):
    c = sea_pic(w, h)
    beam = c["layers"].pop(5)
    c["layers"].insert(5, {"text": "Anikit Video", "font": SERIF, "weight": 600, "size": round(w * 0.12), "color": "#07080a",
                           "letterSpacing": "-0.02em", "x": w * 0.06, "y": h * 0.36, "anchor": "left"})
    c["layers"].insert(6, beam)
    return c


CAP_LH = "A lighthouse at dusk, in rain. The beam finds a small boat."
CAP_GL = "Morning. Three glasses on an oak table; the window light moves."
CAP_INK = "One drop of blue ink falling through still water."
CAP_TY = "The word Rain, letter by letter, the way rain arrives."

comps = {
    "seaL": sea_pic(1620, 870), "seaS": sea_pic(720, 387),
    "lh": sheet("seaL", CAP_LH, 2400), "lhS": sheet("seaS", CAP_LH, 1200),
    "seaN": name_pic(1620, 870), "nameP": sheet("seaN", "anikit-video render spec.json", 2400),
}


# ---------------------------------------------------------------- sheet + develop helpers
def place(comp, W, x, y, s, rot, extra=None, shadow=True, trim=0.0, speed=None, blend=None):
    """A sheet in the tray. x, y, s, rot may be expressions ("=...") in scene time t."""
    H = W * 2 / 3
    out = []
    if shadow:
        out.append({"shape": "rect", "width": W, "height": H, "fill": "#000", "opacity": 0.6, "blur": 26,
                    "x": x, "y": y, "scale": s, "rotation": rot})
    lay = {"composition": comp, "width": W, "height": H, "x": x, "y": y, "scale": s, "rotation": rot}
    if trim:
        lay["trimStart"] = trim
    if speed is not None:
        lay["speed"] = speed
    if blend:
        lay["blend"] = blend
    if extra:
        lay.update(extra)
    out.append(lay)
    return out


def num(v):
    return v[1:] if isinstance(v, str) and v.startswith("=") else str(v)


def develop(W, x, y, s, rot, dev):
    H = W * 2 / 3
    return {"type": "shader", "src": "src/develop.glsl", "filter": True,
            "uniforms": {"dev": dev, "cx": x, "cy": y, "hw": f"=({num(s)})*{W/2}", "hh": f"=({num(s)})*{H/2}", "rot": rot}}


def tray(room=0, lights=1, lx=0.16, ly=0.98, slosh=1.0, wave=-1.0, seed=0.0, extra=None):
    u = {"room": room, "lights": lights, "lx": lx, "ly": ly, "slosh": slosh, "wave": wave, "seed": seed}
    if extra:
        u.update(extra)
    return {"type": "shader", "src": "src/darkroom.glsl", "filter": True, "uniforms": u}


BG = "#090707"
scenes = []

# S1 · 0–5 s · bars 1–2 · macro on the agent's words developing, continuous pull-back to the whole (still) print
x1, y1 = "=1480 - 520*smoothstep(0, 5, t)", "=-170 + 690*smoothstep(0, 5, t)"
s1, r1 = "=1.2 - 0.56*smoothstep(0, 5, t)", "=-7 + 4*smoothstep(0, 5, t)"
dev1 = "=clamp((t+0.9)/4.4, 0, 1)"
scenes.append({"duration": 5.0, "background": BG, "layers":
               place("lh", 2400, x1, y1, s1, r1, speed=0.0) + [
                   develop(2400, x1, y1, s1, r1, dev1),
                   tray(wave="=clamp(t/1.6, 0, 1)*1.0", seed=1.0)]})

# S2 · 5–7.5 s · bar 3 · it moves: closer, the other angle, push toward the lantern; the cello enters
x2, y2 = "=820 - 60*t", "=560 + 20*t"
s2, r2 = "=0.9 + 0.08*t", "=4 - 0.6*t"
scenes.append({"duration": 2.5, "background": BG, "layers":
               place("lh", 2400, x2, y2, s2, r2, trim=5.0) + [tray(lx=0.2, seed=2.0)]})

# S3 · 7.5–11.25 s · bars 4–5½ · a second sheet of the same sentence develops beside the first:
# the two films play in lockstep (same trim, same frame) — the camera tracks across both
xa3, ya3, ra3 = "=640 - 40*t", "=420 + 6*t", -5
xb3, yb3, rb3 = "=1500 - 40*t", "=690 + 6*t", 4
s3 = 0.5
dev3 = "=clamp((t-0.1)/1.6, 0, 1)"
scenes.append({"duration": 3.75, "background": BG, "layers":
               place("lh", 2400, xa3, ya3, s3, ra3, trim=7.5) +
               place("lh", 2400, xb3, yb3, s3, rb3, trim=7.5, extra={"sfx": {"in": "paper"}}) + [
                   develop(2400, xb3, yb3, s3, rb3, dev3), tray(lx=0.12, ly=0.95, seed=3.0)]})

# S4 · 11.25–15 s · bars 5½–6 · wide: a tray of nine sheets of the same sentence, all in sync
l4 = []
for i in range(9):
    cx, cy = 360 + (i % 3) * 600, 190 + (i // 3) * 350
    r = [-3, 2, -1, 3, -2, 1, 2, -3, 0][i]
    l4 += place("lhS", 1200, f"={cx} + 8*sin(t*0.9 + {i})", f"={cy} + 6*cos(t*0.8 + {i*1.7})", 0.44, f"={r} + 0.8*sin(t*0.7 + {i})", trim=11.25)
scenes.append({"duration": 3.75, "background": BG,
               "camera": {"animate": [{"from": {"zoom": 1.55, "rotation": 4}, "to": {"zoom": 1.0, "rotation": 0}, "duration": 3.75, "ease": "outCubic"}]},
               "layers": l4 + [tray(lx=0.18, ly=1.0, slosh=1.2, seed=4.0)]})

# S5 · 15–18.75 s · bars 7–8½ · the twins: a second print of the same sentence slides onto the first
# as a ghost and, in register, becomes one image.
xa, ya, sa, ra = 960, 430, 0.56, -2
xb = "=960 + 1100*pow(1 - smoothstep(0.1, 1.5, t), 3)"
yb = "=430 - 120*pow(1 - smoothstep(0.1, 1.5, t), 3)"
rb = "=-2 + 9*pow(1 - smoothstep(0.1, 1.5, t), 3)"
scenes.append({"duration": 3.75, "background": BG, "layers":
               place("lh", 2400, xa, ya, sa, ra, trim=15.0) +
               place("lh", 2400, xb, yb, sa, rb, trim=15.0, shadow=False,
                     extra={"sfx": {"in": "paper"}, "opacity": "=0.5 + 0.5*smoothstep(1.45, 1.6, t)"}) + [
                   {"text": "*Same words. Same film.*", "font": SERIF, "size": 78, "color": "#EDE6DA",
                    "x": 960 - 2400 * 0.56 / 2 + 40, "y": 965, "anchor": "left", "rotation": -1,
                    "start": 1.5, "enter": {"preset": "fade", "duration": 0.5}},
                   tray(lx=0.16, ly=0.98, seed=5.0)]})

# stop · 18.75–20 s · the safelight dims out over the print (silence)
scenes.append({"duration": 1.25, "background": "#000", "layers":
               place("lh", 2400, 960, 430, 0.56, -2, trim=18.75) + [
                   {"text": "*Same words. Same film.*", "font": SERIF, "size": 78, "color": "#EDE6DA",
                    "x": 960 - 2400 * 0.56 / 2 + 40, "y": 965, "anchor": "left", "rotation": -1},
                   tray(lights="=1 - smoothstep(0, 1.2, t)", seed=6.0)]})

# S6 · 20–25 s · bars 9–10 · room light: the tray of identical prints in true colour, close and turned
l6 = []
for i in range(9):
    cx, cy = 360 + (i % 3) * 600, 190 + (i // 3) * 350
    r = [-3, 2, -1, 3, -2, 1, 2, -3, 0][i]
    l6 += place("lhS", 1200, f"={cx} + 8*sin(t*0.9 + {i})", f"={cy} + 6*cos(t*0.8 + {i*1.7})", 0.44, f"={r} + 0.8*sin(t*0.7 + {i})", trim=20.0)
scenes.append({"duration": 5.0, "background": BG,
               "camera": {"animate": [{"from": {"zoom": 1.9, "rotation": -14, "x": 200, "y": 120}, "to": {"zoom": 1.45, "rotation": -9, "x": 120, "y": 60}, "duration": 5.0, "ease": "inOutSine"}]},
               "layers": l6 + [tray(room=1, lights="=smoothstep(0, 0.08, t)", lx=0.16, ly=0.98, slosh=0.8, seed=7.0)]})

# S7 · 25–30 s · bars 11–12 · the last print develops: the name stands in its picture; its caption is the command
x7 = "=960 - 500*pow(1 - smoothstep(0, 0.7, t), 3)"
y7 = "=520 + 420*pow(1 - smoothstep(0, 0.7, t), 3)"
s7, r7 = "=0.66 + 0.012*t", "=-5*pow(1 - smoothstep(0, 0.7, t), 3) - 1"
dev7 = "=clamp((t-0.3)/2.2, 0, 1)"
scenes.append({"duration": 5.0, "background": BG, "layers":
               place("nameP", 2400, x7, y7, s7, r7, extra={"sfx": {"in": "paper"}}) + [
                   develop(2400, x7, y7, s7, r7, dev7),
                   tray(room=1, lx=0.16, ly=0.98, slosh=0.6, wave="=clamp((t-0.1)/1.8, 0, 1)", seed=8.0)]})

# ---------------------------------------------------------------- sound
ticks = [{"sfx": "tick", "at": round(0.625 * i, 3), "gain": -14, "x": 0.3} for i in range(8)]
cues = ticks + [
    {"sfx": "droplet", "at": 0.25, "gain": -8},
    {"sfx": "click", "at": 18.75, "gain": -2, "x": -0.4},
    {"sfx": "click", "at": 20.0, "gain": -4, "x": -0.4, "pitch": -3},
    {"sfx": "droplet", "at": 25.3, "gain": -9},
]

spec = {
    "size": "16:9",
    "intent": {"pace": "medium", "genre": "launch"},
    "music": {
        "style": "piano", "bpm": 96, "key": "Dm", "mood": "tender, building",
        "instruments": {"lead": "cello"},
        "sections": [{"name": "intro", "bars": 2, "energy": 0.15},
                     {"name": "verse", "bars": 4, "energy": 0.5},
                     {"name": "build", "bars": 2, "energy": 0.8},
                     {"name": "payoff", "bars": 4, "energy": 1.0, "stop": 2}],
        "hits": ["@bar9"], "end": "ring", "followCuts": False},
    "sound": {"palette": "organic", "ambience": "room", "space": "room", "density": 0.4, "cues": cues},
    "compositions": comps,
    "scenes": scenes,
}

json.dump(spec, open("spec.json", "w"), indent=1)
print("wrote spec.json", sum(s["duration"] for s in scenes), "s")
