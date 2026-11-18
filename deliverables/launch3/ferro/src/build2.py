"""Pass 2: the balance. Two Ferro pans weigh one classic skillet."""
import json, sys, math

Y0, L = 0.62, 0.5          # beam pivot height, half-length
TRAY_UP = 0.075            # tray top above beam axis
BRASS = {"base": "#B8893A", "metallic": 1.0, "roughness": 0.28, "imperfections": 0.35}
DARK = {"base": "#1D1B1A", "metallic": 0.3, "roughness": 0.55, "imperfections": 0.3}
FERRO = {"texture": "ambientcg:Metal063", "scale": 0.35, "color": "#2B2826", "roughness": 0.3, "metallic": 0.55,
         "imperfections": 0.25}
CLASSIC = {"texture": "ambientcg:Metal063", "scale": 0.5, "color": "#8A8078", "roughness": 0.85, "metallic": 0.25,
           "imperfections": 0.7}
CONC = {"texture": "ambientcg:Concrete034", "scale": 0.8}

T_CLASSIC, T_SECOND = 0.45, 8.6
def theta(g):
    if g < T_CLASSIC: return -13.0
    if g < T_SECOND:
        u = g - T_CLASSIC
        return 13.0 - 26.0 * math.exp(-7 * u) * math.cos(5 * u)
    u = g - T_SECOND
    if g > 12.2: return 0.0
    return 13.0 * math.exp(-1.5 * u) * math.cos(2.7 * u) * (1 - max(0, (g - 11.6) / 0.6))

def tray_xy(side, g):
    th = math.radians(theta(g))
    return side * L * math.cos(th), Y0 + side * L * math.sin(th)

def dense(t0, t1, f, step=1/30):
    ks, n = [], max(2, int((t1 - t0) / step))
    for i in range(n + 1):
        lt = i * (t1 - t0) / n
        k = f(t0 + lt); k["t"] = round(lt, 4); k["ease"] = "linear"; ks.append(k)
    return ks

def ferro_pan(name):
    prof = [[0, 0], [0.102, 0], [0.112, 0.004], [0.118, 0.014], [0.124, 0.043], [0.1215, 0.047],
            [0.118, 0.046], [0.1135, 0.016], [0.107, 0.008], [0.095, 0.0055], [0, 0.0055]]
    return [{"type": "lathe", "name": name, "segments": 160, "smooth": True, "points": prof, "material": FERRO},
            {"type": "tube", "points": [[0.118, 0.036, 0], [0.17, 0.045, 0], [0.23, 0.052, 0], [0.275, 0.056, 0]],
             "radius": 0.0095, "material": FERRO, "rel": True},
            {"type": "box", "size": [0.04, 0.008, 0.05], "radius": 0.007, "pos": [-0.135, 0.04, 0], "material": FERRO, "rel": True}]

def classic_pan():
    prof = [[0, 0], [0.122, 0], [0.135, 0.007], [0.143, 0.022], [0.15, 0.058], [0.146, 0.064],
            [0.137, 0.063], [0.131, 0.025], [0.122, 0.016], [0.105, 0.013], [0, 0.013]]
    return [{"type": "lathe", "name": "classic", "segments": 160, "smooth": True, "points": prof, "material": CLASSIC},
            {"type": "box", "size": [0.16, 0.022, 0.034], "radius": 0.008, "pos": [0.215, 0.052, 0], "rot": [0, 0, 8],
             "material": CLASSIC, "rel": True},
            {"type": "box", "size": [0.05, 0.016, 0.06], "radius": 0.009, "pos": [-0.165, 0.055, 0], "material": CLASSIC, "rel": True}]

def place(parts, base, keysf, yaw=0):
    """parts at base pos; keysf(g)->offset (dx,dy) added; all parts share keys (translation only)."""
    out = []
    for p in parts:
        q = dict(p); rel = q.pop("rel", False)
        pp = q.get("pos", [0, 0, 0])
        if yaw:
            c, s = math.cos(math.radians(yaw)), math.sin(math.radians(yaw))
            pp = [pp[0] * c - pp[2] * s, pp[1], pp[0] * s + pp[2] * c]
            r = list(q.get("rot", [0, 0, 0])); r[1] += -yaw; q["rot"] = r
        q["pos"] = [base[0] + pp[0], base[1] + pp[1], base[2] + pp[2]]
        q["keys"] = keysf
        out.append(q)
    return out

def build_scene(g0, dur, camera, extra=()):
    rest_y = Y0 + TRAY_UP
    def tray_keys(side):
        bx, by = side * L, Y0
        def f(g):
            x, y = tray_xy(side, g); return {"pos": [round(x - bx, 4), round(y - by, 4), 0]}
        return dense(g0, g0 + dur, f)
    beam_keys = dense(g0, g0 + dur, lambda g: {"rot": [0, 0, round(theta(g), 3)]})
    o = []
    # plinth + post
    o.append({"type": "box", "size": [0.9, 0.07, 0.3], "radius": 0.01, "pos": [0, 0.035, 0], "material": DARK})
    o.append({"type": "box", "size": [0.05, Y0 - 0.06, 0.05], "radius": 0.006, "pos": [0, 0.06 + (Y0 - 0.06) / 2 - 0.02, 0], "material": DARK})
    o.append({"type": "box", "name": "beam", "size": [2 * L + 0.04, 0.05, 0.03], "radius": 0.006, "pos": [0, Y0, 0],
              "material": BRASS, "keys": beam_keys})
    o.append({"type": "text", "name": "stamp", "text": "HALF THE WEIGHT", "font": "Alfa Slab One", "height": 0.026,
              "depth": 0.004, "bevel": 0.0008, "tracking": 0.06, "pos": [0, Y0 - 0.001, 0.016], "material": DARK, "keys": beam_keys})
    for side in (-1, 1):
        tk = tray_keys(side)
        o.append({"type": "box", "size": [0.02, TRAY_UP - 0.01, 0.02], "pos": [side * L, Y0 + TRAY_UP / 2, 0], "material": BRASS, "keys": tk})
        o.append({"type": "cylinder", "radius": 0.17, "height": 0.01, "pos": [side * L, Y0 + TRAY_UP, 0], "material": BRASS, "keys": tk})
    top = Y0 + TRAY_UP + 0.005
    # Ferro 1 on right tray
    def f1(g):
        x, y = tray_xy(1, g); return {"pos": [round(x - L, 4), round(y - Y0, 4), 0]}
    o += place(ferro_pan("ferro1"), [L, top, 0], dense(g0, g0 + dur, f1), yaw=-90)
    # classic drops onto left tray at T_CLASSIC
    def fc(g):
        x, y = tray_xy(-1, g)
        drop = 0.0 if g >= T_CLASSIC else 0.22 * (1 - (g / T_CLASSIC) ** 2)
        return {"pos": [round(x + L, 4), round(y - Y0 + drop, 4), 0]}
    o += place(classic_pan(), [-L, top, 0], dense(g0, g0 + dur, fc), yaw=90)
    # second Ferro lowered/dropped onto first at T_SECOND
    if g0 + dur > T_SECOND - 1.4:
        def f2(g):
            x, y = tray_xy(1, g)
            a = T_SECOND - 1.4
            if g < T_SECOND:
                u = max(0.0, (g - a) / 1.4); drop = 0.9 * (1 - u * u)
            else: drop = 0.0
            return {"pos": [round(x - L, 4), round(y - Y0 + drop, 4), 0]}
        o += place(ferro_pan("ferro2"), [L, top + 0.016, 0], dense(g0, g0 + dur, f2), yaw=-70)
    o += list(extra)
    walls = [{"type": "box", "size": [10, 5, 0.05], "pos": [0, 2.5, -1.4], "material": CONC},
             {"type": "box", "size": [0.05, 5, 10], "pos": [-2.2, 2.5, 0], "material": CONC}]
    props = {"env": {"preset": "cyc"}, "set": {"preset": "sweep", "color": "#CFC8BC", "size": 30},
             "rig": {"preset": "window", "angle": 50, "height": 40, "warmth": 0.15, "ratio": 5, "key": 1.6, "gobo": "window"},
             "camera": camera, "objects": o}
    return {"duration": dur, "layers": [{"component": "scene3d", "props": props}]}

def cam(*keys, shake="breath"):
    return {"shake": shake, "keys": list(keys)}

S = []
S.append(build_scene(0, 2, cam({"t": 0, "pos": [-0.22, 0.9, 0.62], "target": [-0.5, 0.66, 0], "focal": 50, "fstop": 4},
                               {"t": 2, "pos": [-0.25, 0.86, 0.58], "target": [-0.5, 0.64, 0], "focal": 50, "fstop": 4})))
S.append(build_scene(2, 2.5, cam({"t": 0, "pos": [0.45, 0.6, 1.55], "target": [0, 0.62, 0], "focal": 35, "fstop": 5.6},
                                 {"t": 2.5, "pos": [0.4, 0.6, 1.4], "target": [0, 0.62, 0], "focal": 35, "fstop": 5.6})))
S.append(build_scene(4.5, 2.5, cam({"t": 0, "pos": [0.78, 0.86, 0.98], "target": [0.12, 0.68, 0], "focal": 32, "fstop": 3.5, "focus": "ferro1"},
                                   {"t": 2.5, "pos": [0.72, 0.86, 0.9], "target": [0.12, 0.68, 0], "focal": 32, "fstop": 3.5, "focus": "ferro1"})))
S.append(build_scene(7, 2.5, cam({"t": 0, "pos": [0.95, 1.2, 0.62], "target": [0.5, 0.75, 0], "focal": 40, "fstop": 4.5},
                                 {"t": 2.5, "pos": [0.92, 1.12, 0.58], "target": [0.5, 0.72, 0], "focal": 40, "fstop": 4.5}, shake="handheld")))
S.append(build_scene(9.5, 2.5, cam({"t": 0, "pos": [-0.55, 0.4, 1.38], "target": [0, 0.66, 0], "focal": 32, "fstop": 5.6},
                                   {"t": 2.5, "pos": [-0.85, 0.42, 1.02], "target": [0, 0.64, 0], "focal": 32, "fstop": 5.6})))
S.append(build_scene(12, 2, cam({"t": 0, "pos": [-0.14, 0.64, 0.6], "target": [0.0, 0.615, 0.016], "focal": 50, "fstop": 2.8, "focus": "stamp"},
                                {"t": 2, "pos": [0.08, 0.64, 0.55], "target": [0.01, 0.615, 0.016], "focal": 50, "fstop": 2.8, "focus": "stamp"})))
# S7: letters drop onto the beam centre; CTA raised on the plinth.
let = []
xs = [-0.155, -0.08, 0.0, 0.081, 0.16]
for i, ch in enumerate("FERRO"):
    t0 = 0.35 + 0.22 * i
    ks = [{"t": 0, "pos": [0, 0.8, 0], "rot": [0, 0, 0], "ease": "linear"}, {"t": t0, "pos": [0, 0.8, 0], "ease": "in"},
          {"t": t0 + 0.4, "pos": [0, 0, 0], "ease": "linear"}, {"t": t0 + 0.47, "pos": [0, 0.008, 0], "ease": "out"},
          {"t": t0 + 0.55, "pos": [0, 0, 0], "ease": "in"}]
    let.append({"type": "text", "name": f"L{i}", "text": ch, "font": "Alfa Slab One", "height": 0.1, "depth": 0.03,
                "bevel": 0.0015, "pos": [xs[i], Y0 + 0.025 + 0.05, 0], "material": FERRO, "keys": ks})
let.append({"type": "text", "name": "cta", "text": "BACK IT ON KICKSTARTER", "font": "Alfa Slab One", "height": 0.04,
            "depth": 0.005, "bevel": 0.0008, "tracking": 0.03, "pos": [0, 0.035, 0.151], "material": {"base": "#EADBB8", "roughness": 0.45},
            "keys": [{"t": 0, "pos": [0, -0.04, 0]}, {"t": 2.6, "pos": [0, -0.04, 0], "ease": "linear"}, {"t": 3.1, "pos": [0, 0, 0], "ease": "out"}]})
S.append(build_scene(14, 6, cam({"t": 0, "pos": [0, 0.72, 1.45], "target": [0, 0.42, 0], "focal": 35, "fstop": 6.3, "focus": "L2"},
                                {"t": 6, "pos": [0, 0.66, 1.22], "target": [0, 0.42, 0], "focal": 35, "fstop": 6.3, "focus": "L2", "ease": "out"}), extra=let))

spec = {"size": "1:1", "fps": 30,
        "music": {"style": "acoustic", "instrumentation": "band", "bpm": 120, "key": "D",
                  "sections": [{"name": "intro", "bars": 2, "energy": 0.6}, {"name": "build", "bars": 4, "energy": 0.8},
                               {"name": "swing", "bars": 1, "energy": 0.9}, {"name": "level", "bars": 3, "energy": 1, "stop": 4}],
                  "hits": ["@bar8"], "end": "ring", "followCuts": False},
        "sound": "auto", "post": {"grain": 0.04, "vignette": 0.12}, "scenes": S}
json.dump(spec, open(sys.argv[1], "w"), indent=1)
