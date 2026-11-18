#!/usr/bin/env python3
"""Railyard launch film, pass 2: "Already settled".
An hourglass is turned over and the sand (the money, in cobalt) is already in the
lower bulb. World: a white-marble banking counter, brass and clear glass, afternoon
sun from a tall window (photographed HDRI). Writes ../spec.json."""
import json, math, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..")

BPM = 96
BEAT = 60 / BPM
BAR = 4 * BEAT

BRASS = {"extends": "gold", "base": "#B9925A", "roughness": 0.32, "imperfections": 0.5}
GLASS = {"extends": "glass", "imperfections": 0.25}
SAND = {"base": "#2346B0", "roughness": 0.97, "imperfections": 0.6}
MARBLE = {"texture": "ambientcg:Marble021", "scale": 1.5}
FLIP = 0.42  # seconds per turn

H = 0.114  # disc offset from the neck
GLASS_PROFILE = [[0.008, -0.106], [0.03, -0.1], [0.041, -0.078], [0.038, -0.05], [0.02, -0.02], [0.0045, -0.004],
                 [0.0045, 0.004], [0.02, 0.02], [0.038, 0.05], [0.041, 0.078], [0.03, 0.1], [0.008, 0.106]]
SAND_PROFILE = [[0.0, -0.104], [0.026, -0.101], [0.036, -0.088], [0.039, -0.074], [0.03, -0.066], [0.014, -0.06], [0.0, -0.057]]


def theta(t0s):
    """Rotation (deg) after a list of turn start times: each turn adds 180."""
    parts = []
    for t0 in t0s:
        p = f"clamp((t-{t0:.4f})/{FLIP},0,1)"
        parts.append(f"180*({p}*{p}*(3-2*{p}))")
    return "(" + "+".join(parts) + ")" if parts else "0"


def lift(t0s, amt=0.05):
    parts = [f"{amt}*sin(3.14159*clamp((t-{t0:.4f})/{FLIP},0,1))" for t0 in t0s]
    return "(" + "+".join(parts) + ")" if parts else "0"


def hourglass(name, cx, cz, t0s, freeze=None, cy=None, ry=0.0):
    """Objects for one hourglass standing on y=0 at (cx, cz), turned over at each t0.
    freeze=(t, deg): hold at that angle from time t (for the hard stop)."""
    cy = 0.12 if cy is None else cy
    th = theta(t0s)
    lf = lift(t0s)
    if freeze is not None:
        th = f"{freeze}"
        lf = "0.07"
    TH = f"({th})"
    c = f"cos({TH}*0.0174533)"
    s = f"sin({TH}*0.0174533)"
    Y = f"({cy}+{lf})"
    objs = []
    # discs (offset along the hourglass axis)
    for k, oy in (("bot", -H), ("top", H)):
        objs.append({"type": "cylinder", "name": f"{name}_{k}", "radius": 0.058, "height": 0.012,
                     "pos": [f"={cx}-({oy})*{s}", f"={Y}+({oy})*{c}", cz], "rot": [0, 0, f"={TH}"], "material": BRASS})
    # three rods
    for j, a in enumerate((90, 210, 330)):
        ox = 0.048 * math.cos(math.radians(a))
        oz = 0.048 * math.sin(math.radians(a))
        objs.append({"type": "cylinder", "radius": 0.0035, "height": 0.216,
                     "pos": [f"={cx}+({ox:.4f})*{c}", f"={Y}+({ox:.4f})*{s}", round(cz + oz, 4)],
                     "rot": [0, 0, f"={TH}"], "material": BRASS})
    # glass (symmetric: rotate about the neck)
    objs.append({"type": "lathe", "name": f"{name}_glass", "points": GLASS_PROFILE, "segments": 64,
                 "pos": [cx, f"={Y}", cz], "rot": [0, 0, f"={TH}"], "material": GLASS})
    # sand: always settled in the lower bulb. Rides with the glass for the first part of a turn,
    # then is already down (the cut happens at 60% of each turn, mid-air).
    if freeze is not None:
        objs.append({"type": "lathe", "points": SAND_PROFILE, "segments": 48, "pos": [cx, f"={Y}", cz], "rot": [0, 0, f"={TH}"], "material": SAND})
        return objs
    # phase: fraction of current turn; riding when 0<p<0.6, otherwise upright
    ride_terms = [f"step2((t-{t0:.4f})/{FLIP})" for t0 in t0s]
    ride = "max(" + ",".join(ride_terms) + ",0)" if len(ride_terms) > 1 else (ride_terms[0] if ride_terms else "0")
    ride = ride.replace("step2(", "rideok(")
    # implement rideok(x) = 1 if 0<x<0.6 else 0, with clamps
    def rideok(x):
        return f"(clamp(({x})*1000,0,1)*clamp((0.6-({x}))*1000,0,1))"
    rt = [rideok(f"(t-{t0:.4f})/{FLIP}") for t0 in t0s]
    rexpr = rt[0] if rt else "0"
    for r in rt[1:]:
        rexpr = f"max({rexpr},{r})"
    # riding sand: rotates with glass
    objs.append({"type": "lathe", "points": SAND_PROFILE, "segments": 48,
                 "pos": [cx, f"={Y}", cz], "rot": [0, 0, f"={TH}"],
                 "scale": "=[" + ",".join([f"0.001+0.999*{rexpr}"]*3) + "]", "material": SAND})
    # settled sand: upright, follows the glass position but not its rotation
    objs.append({"type": "lathe", "name": f"{name}_sand", "points": SAND_PROFILE, "segments": 48,
                 "pos": [cx, f"={Y}", cz], "scale": "=[" + ",".join([f"1-0.999*{rexpr}"]*3) + "]", "material": SAND})
    return objs


def key(t, pos, target, focus=None, fstop=4.0, focal=50, shutter=0.5):
    k = {"t": t, "pos": pos, "target": target, "fstop": fstop, "focal": focal, "shutter": shutter}
    if focus:
        k["focus"] = focus
    return k


def scene(objs, cam, exposure=0.0, rotate=200, brand=None):
    objs = [{"type": "box", "name": "counter", "size": [8, 0.1, 4], "pos": [0, -0.05, 0], "material": MARBLE}] + objs
    if brand:
        objs.append(brand)
    return {"component": "scene3d", "props": {
        "env": {"hdri": "polyhaven:hdri/photo_studio_loft_hall", "rotate": rotate, "exposure": exposure, "background": "blur"},
        "camera": {"shake": "breath", "keys": cam},
        "objects": objs,
    }}


# ------------------------------------------------------------------ shots
def shot1():
    """Hook: one glass already mid-turn at frame 0; lands settled. Turned again on beat 3. Macro, low."""
    d = BAR
    t0s = [-0.12, BEAT * 2]
    objs = hourglass("a", 0, 0, t0s)
    cam = [key(0, [0.30, 0.17, 0.66], [0, 0.11, 0], "a_glass", 2.8, 60),
           key(d, [0.24, 0.16, 0.57], [0, 0.11, 0], "a_glass", 2.8, 60)]
    return d, scene(objs, cam)


def shot2():
    """A row of five on the counter, each turned on the beat, each lands settled. Side truck."""
    d = BAR
    objs = []
    for i in range(5):
        objs += hourglass(f"r{i}", -0.6 + i * 0.3, 0, [BEAT * (i * 0.75) + 0.1])
    cam = [key(0, [-0.35, 0.22, 1.15], [-0.1, 0.11, 0], "r2_glass", 4, 40),
           key(d, [0.05, 0.22, 1.1], [0.15, 0.11, 0], "r2_glass", 4, 40)]
    return d, scene(objs, cam, rotate=170)


def shot3():
    """A line of twelve receding along the counter: a domino wave away from camera. Rack along."""
    d = BAR
    objs = []
    for i in range(12):
        objs += hourglass(f"l{i}", 0.0 + i * 0.16, -i * 0.22, [0.25 + i * 0.1])
    cam = [key(0, [1.25, 0.3, 0.35], [0.35, 0.08, -0.7], "l2_glass", 2.8, 40),
           key(d * 0.5, [1.3, 0.3, 0.3], [0.6, 0.08, -0.95], "l5_glass", 2.8, 40),
           key(d, [1.35, 0.3, 0.25], [0.9, 0.08, -1.3], "l8_glass", 2.8, 40)]
    return d, scene(objs, cam, rotate=230)


def shot4():
    """Scale: a field of glasses on the counter turning in a radial wave from the centre. High plan."""
    d = BAR
    objs = []
    n = 0
    for r in range(-3, 4):
        for c in range(-5, 6):
            x, z = c * 0.17, r * 0.17
            dist = math.hypot(c, r)
            objs += hourglass(f"g{n}", round(x, 3), round(z, 3), [0.2 + dist * 0.09])
            n += 1
    cam = [key(0, [0.15, 1.75, 0.75], [0, 0, -0.02], None, 8, 40),
           key(d, [0.12, 1.6, 0.7], [0, 0, -0.02], None, 8, 40)]
    return d, scene(objs, cam, rotate=200)


def shot5():
    """Hard stop: one glass frozen mid-turn, horizontal, in the air. Silence."""
    d = BAR / 2
    objs = hourglass("f", 0, 0, [], freeze=90)
    cam = [key(0, [0.05, 0.2, 0.42], [0, 0.18, 0], "f_glass", 2.8, 60, 0.1),
           key(d, [0.04, 0.2, 0.40], [0, 0.18, 0], "f_glass", 2.8, 60, 0.1)]
    return d, scene(objs, cam, rotate=200)


def shot6():
    """Payoff: it lands, settled; the camera settles back onto the name inlaid in the marble."""
    d = BAR * 1.5
    objs = hourglass("z", 0.0, -0.06, [-FLIP * 0.5])
    brand = {"type": "plane", "size": [0.56, 0.14], "pos": [0, 0.0006, 0.12], "rot": [-90, 0, 0],
             "texture": "#brand", "material": "decal", "thin": True, "shadow": False}
    cam = [key(0, [0.06, 0.16, 0.36], [0, 0.13, -0.06], "z_glass", 2.8, 60),
           key(1.6, [0.02, 0.40, 0.70], [0, 0.05, 0.02], "z_glass", 5.6, 45),
           key(d, [0.015, 0.41, 0.67], [0, 0.045, 0.02], "z_glass", 5.6, 45)]
    return d, scene(objs, cam, rotate=200, brand=brand)


def music():
    return {
        "bpm": BPM, "key": "D", "end": "ring",
        "tracks": [
            # the turns: a low wooden knock + pizzicato on each landing (the settle)
            {"inst": "strings", "notes": "o3 [D A]/1 | [D A]/1 | [Bb, F D]/1 | [G, D G]/2 [A, E A]/2 | r1 | [D A D' F#']/1 | _2", "gain": -10, "reverb": 0.45},
            {"inst": "piano", "notes": "o5 r8 D/8! r4 A/8 r8 D'/4 | r8 D/8 r8 A/8 r8 D'/8 r8 A'/8 | D/16 F/16 A/16 D'/16 r4 F/16 A/16 D'/16 F'/16 r4 | D'/8 A/8 F/8 D/8 E/8 A/8 C#'/8 E'/8 | r1 | r4 [F# A D']/2. | _2", "gain": -6, "reverb": 0.35},
            {"inst": "cello", "notes": "o2 D/1 | D/1 | Bb,/1 | G,/2 A,/2 | r1 | D/1 | _2", "gain": -8},
            {"inst": "drums:brush", "drums": {"kick": "x.......x.......", "tom": "........x.......", "shaker": "..x...x...x...x."}, "bars": 4, "gain": -10},
        ],
        "master": {"lufs": -15, "reverb": "hall"},
    }


def build(only=None):
    shots = [shot1, shot2, shot3, shot4, shot5, shot6]
    scenes = []
    for fn in shots:
        d, layer = fn()
        scenes.append({"duration": round(d, 4), "background": "#D8D2C6", "layers": [layer]})
    spec = {
        "size": "16:9", "fps": 30,
        "compositions": {
            "brand": {"size": [2000, 500], "duration": 4, "layers": [
                {"text": "RAILYARD", "font": "Cormorant Garamond", "weight": 700, "italic": False, "letterSpacing": 0.12, "size": 240, "x": 1000, "y": 190, "anchor": "center", "color": "#1C2F86"},
                {"text": "One API call. Settled instantly.", "font": "Cormorant Garamond", "weight": 700, "size": 132, "x": 1000, "y": 400, "anchor": "center", "color": "#1C2F86"},
            ]}},
        "music": music(),
        "sound": "auto",
        "post": {"grain": 0.05, "vignette": 0.12},
        "scenes": scenes,
    }
    return spec


if __name__ == "__main__":
    spec = build()
    with open(os.path.join(OUT, "spec.json"), "w") as f:
        json.dump(spec, f, indent=1)
    print("wrote spec.json", sum(s["duration"] for s in spec["scenes"]))
