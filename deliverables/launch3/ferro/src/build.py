import json, sys, math
sys.path.insert(0, __file__.rsplit("/", 1)[0])
from world import *

BASE = {"set": {"preset": "sweep", "color": "#C4532E", "size": 30}, "env": {"preset": "cyc"},
        "rig": {"preset": "window", "angle": 55, "height": 50, "warmth": 0.3, "ratio": 5, "key": 1.6, "gobo": "window"}}

def keys_hop(hops, u0=0.0):
    """hops: list of (t0, dur, h, spin). Offsets from rest; stays at rest between hops."""
    ks = [{"t": 0, "pos": [0, 0, 0], "rot": [0, 0, 0], "ease": "linear"}]
    acc = [0, 0, 0]
    for j, (t0, dur, h, spin) in enumerate(hops):
        n = max(10, int(dur * 30))
        start = u0 if j == 0 else 0.0
        if t0 > 0:
            ks.append({"t": t0, "pos": [0, 0, 0], "rot": list(acc), "ease": "linear"})
        else:
            ks = []
        for i in range(0, n + 1):
            u = start + (1 - start) * i / n
            r = [acc[k] + spin[k] * u for k in range(3)]
            ks.append({"t": round(t0 + (u - start) * dur / (1 - start) if start else t0 + u * dur, 4),
                       "pos": [0, round(4 * h * u * (1 - u), 4), 0], "rot": [round(x, 2) for x in r], "ease": "linear"})
        acc = [acc[k] + spin[k] for k in range(3)]
    # sort/dedupe by t
    out = {}
    for k in ks: out[k["t"]] = k
    return [out[t] for t in sorted(out)]

def with_keys(objs, keys):
    for o in objs:
        o["keys"] = [dict(k) for k in keys]
    return objs

def pan_jolt(times, amp=0.012):
    ks = [{"t": 0, "pos": [0, 0, 0], "ease": "linear"}]
    for t in times:
        ks += [{"t": max(0.001, t - 0.08), "pos": [0, 0, 0], "ease": "linear"},
               {"t": t, "pos": [0, amp, 0.0], "ease": "out"},
               {"t": t + 0.12, "pos": [0, 0, 0], "ease": "in"}]
    return ks

def cam(*keys, shake="breath"):
    return {"shake": shake, "keys": list(keys)}

WALLS = [{"type": "box", "size": [10, 5, 0.05], "pos": [0, 2.5, -1.6], "material": {"base": "#A6431F", "roughness": 0.92, "imperfections": 0.15}},
         {"type": "box", "size": [0.05, 5, 10], "pos": [-1.6, 2.5, 0], "material": {"base": "#A6431F", "roughness": 0.92, "imperfections": 0.15}}]
def scene(dur, objects, camera, layers=(), walls=True, **kw):
    p = dict(BASE, camera=camera, objects=objects + ([dict(w) for w in WALLS] if walls else []))
    p.update(kw)
    return {"duration": dur, "layers": [{"component": "scene3d", "props": p}] + list(layers)}

FLOOR = 0.007
def breakfast(seed=0):
    """(objects-list-of-lists, rest positions) — each item list moves as one."""
    items = [
        [pancake([0.0, FLOOR, 0.0])],
        egg([0.05, FLOOR + 0.011, 0.045], "egg1"),
        egg([-0.05, FLOOR + 0.011, -0.04], "egg2"),
        [tomato([0.07, FLOOR + 0.02, -0.06])],
        [tomato([-0.075, FLOOR + 0.02, 0.05])],
        [mushroom([0.0, FLOOR + 0.011, 0.085])],
        [mushroom([-0.02, FLOOR + 0.011, -0.09])],
        [mushroom([0.09, FLOOR + 0.011, 0.0])],
    ]
    return items

SPINS = [(180, 0, 40), (360, 0, -90), (-360, 90, 0), (200, 0, 300), (-220, 60, -300), (300, 0, 200), (-300, 0, 120), (360, 120, 0)]

scenes = []

# S1 0-2: egg hops, top-down. Frame 0 egg mid-air.
o = with_keys(pan_parts(), pan_jolt([0.98], 0.008))
o += with_keys(egg([0.0, FLOOR + 0.0, 0.0]), keys_hop([(0, 0.55, 0.06, (0, 0, 0)), (1.0, 0.75, 0.11, (360, 0, 0))], u0=0.25))
scenes.append(scene(2.0, o, cam({"t": 0, "pos": [0.02, 0.62, 0.002], "target": [0.04, 0, 0], "focal": 50, "fstop": 8},
                                 {"t": 2, "pos": [0.02, 0.56, 0.002], "target": [0.04, 0, 0], "focal": 50, "fstop": 8}), walls=False))

# S2 2-4: pancake flip, low macro, tilt up after it.
o = with_keys(pan_parts(), pan_jolt([0.25], 0.012))
o += with_keys([pancake([0, FLOOR, 0])], keys_hop([(0.25, 1.15, 0.32, (180, 0, 0))]))
scenes.append(scene(2.0, o, cam({"t": 0, "pos": [-0.12, 0.2, 0.44], "target": [0, 0.04, 0], "focal": 50, "fstop": 2.8},
                                 {"t": 0.8, "pos": [-0.12, 0.22, 0.46], "target": [0, 0.24, 0], "focal": 50, "fstop": 2.8, "ease": "inOut"},
                                 {"t": 1.6, "pos": [-0.12, 0.2, 0.44], "target": [0, 0.04, 0], "focal": 50, "fstop": 2.8, "ease": "inOut"},
                                 {"t": 2.0, "pos": [-0.11, 0.19, 0.41], "target": [0, 0.04, 0], "focal": 50, "fstop": 2.8})))

def breakfast_toss(t0, dur, h, stagger=0.03, scale_h=None):
    out = []
    for i, item in enumerate(breakfast()):
        hh = h * (1 + 0.12 * ((i * 37) % 5 - 2) / 2)
        out += with_keys(item, keys_hop([(t0 + stagger * i, dur * hh / h, hh, SPINS[i])]))
    return out

# S3 4-6: whole breakfast lifts, 3/4 high.
o = with_keys(pan_parts(), pan_jolt([0.3], 0.015)) + breakfast_toss(0.3, 1.15, 0.30)
scenes.append(scene(2.0, o, cam({"t": 0, "pos": [0.5, 0.36, 0.62], "target": [0, 0.12, 0], "focal": 40, "fstop": 5.6},
                                 {"t": 2, "pos": [0.44, 0.32, 0.55], "target": [0, 0.12, 0], "focal": 40, "fstop": 5.6})))

# S4 6-8: top-down, it comes at the lens.
o = with_keys(pan_parts(), pan_jolt([0.15], 0.015)) + breakfast_toss(0.15, 1.5, 0.42, 0.02)
scenes.append(scene(2.0, o, cam({"t": 0, "pos": [-0.02, 0.62, 0.002], "target": [-0.02, 0, 0], "focal": 35, "fstop": 8},
                                 {"t": 2, "pos": [-0.02, 0.66, 0.002], "target": [-0.02, 0, 0], "focal": 35, "fstop": 8}, shake="handheld"), walls=False))

# S5 8-11.5: big launch, low crane up; items decelerate and keep rising slowly.
HANG = [  # final hang offsets (dx, y, dz) for each item, roughly a loose cloud
    (-0.10, 0.92, -0.06), (0.16, 1.02, -0.10), (0.0, 1.22, -0.22), (0.20, 0.86, 0.12),
    (-0.14, 1.12, 0.14), (0.08, 0.76, 0.2), (-0.24, 1.0, -0.16), (0.18, 1.14, 0.0)]
def launch_keys(i, t0, T, rest, end, spin):
    ks = [{"t": 0, "pos": [0, 0, 0], "rot": [0, 0, 0], "ease": "linear"},
          {"t": t0, "pos": [0, 0, 0], "rot": [0, 0, 0], "ease": "linear"}]
    n = 40
    for j in range(1, n + 1):
        u = j / n
        e = 1 - (1 - u) ** 3  # decelerating
        ks.append({"t": round(t0 + u * T, 4),
                   "pos": [round((end[0] - rest[0]) * e, 4), round(end[1] * e, 4), round((end[2] - rest[2]) * e, 4)],
                   "rot": [round(s * 0.5 * e, 1) for s in spin], "ease": "linear"})
    return ks
o = with_keys(pan_parts(), pan_jolt([0.35], 0.02))
rests = [(0, 0, 0), (0.05, 0, 0.045), (-0.05, 0, -0.04), (0.07, 0, -0.06), (-0.075, 0, 0.05), (0, 0, 0.085), (-0.02, 0, -0.09), (0.09, 0, 0)]
for i, item in enumerate(breakfast()):
    o += with_keys(item, launch_keys(i, 0.35 + 0.02 * i, 3.15, rests[i], HANG[i], SPINS[i]))
scenes.append(scene(3.5, o, cam({"t": 0, "pos": [0.75, 0.42, 0.95], "target": [0, 0.08, 0], "focal": 32, "fstop": 5.6},
                                 {"t": 0.6, "pos": [0.72, 0.5, 0.95], "target": [0, 0.3, 0], "focal": 32, "fstop": 5.6, "ease": "inOut"},
                                 {"t": 3.5, "pos": [0.36, 1.84, 0.70], "target": [0, 0.88, 0.06], "focal": 32, "fstop": 5.6, "ease": "inOut"})))

# S6 11.5-14: the hang. Items around y~1, slow drift; iron claim floats among them.
o = []
for i, item in enumerate(breakfast()):
    dx, y, dz = HANG[i]
    rx, ry, rz = rests[i]
    for p in item:
        q = dict(p)
        q["pos"] = [p["pos"][0] - rx + dx, p["pos"][1] + y, p["pos"][2] - rz + dz]
        s = SPINS[i]
        q["keys"] = [{"t": 0, "pos": [0, 0, 0], "rot": [s[0] * 0.5, s[1] * 0.5, s[2] * 0.5], "ease": "linear"},
                     {"t": 2.5, "pos": [0, 0.025, 0], "rot": [s[0] * 0.53, s[1] * 0.53, s[2] * 0.53], "ease": "linear"}]
        o.append(q)
TXT = {"texture": "ambientcg:Metal063", "scale": 0.35, "color": "#3A3633", "roughness": 0.4, "metallic": 0.5}
o.append({"type": "text", "name": "claim1", "text": "Cast iron,", "font": "Archivo", "weight": 900, "height": 0.055,
          "depth": 0.014, "bevel": 0.002, "pos": [-0.04, 1.06, 0.16], "rot": [-58, 24, -3], "material": TXT,
          "keys": [{"t": 0, "pos": [0, -0.02, 0], "rot": [0, -6, 0], "ease": "linear"}, {"t": 2.5, "pos": [0, 0.012, 0], "rot": [0, 4, 0]}]})
o.append({"type": "text", "name": "claim2", "text": "half the weight.", "font": "Archivo", "weight": 900, "height": 0.055,
          "depth": 0.014, "bevel": 0.002, "pos": [0.01, 1.0, 0.22], "rot": [-58, 24, 2], "material": TXT,
          "keys": [{"t": 0, "pos": [0, -0.03, 0], "rot": [0, -8, 0], "ease": "linear"}, {"t": 2.5, "pos": [0, 0.02, 0], "rot": [0, 5, 0]}]})
scenes.append(scene(2.5, o, cam({"t": 0, "pos": [0.34, 1.86, 0.66], "target": [0.0, 0.88, 0.08], "focal": 42, "fstop": 5.6, "focus": "claim2"},
                                 {"t": 2.5, "pos": [0.30, 1.80, 0.58], "target": [0.0, 0.88, 0.08], "focal": 42, "fstop": 5.6, "focus": "claim2"})))

# S7 14-16: everything crashes back into the pan on the downbeat.
o = with_keys(pan_parts(), [{"t": 0, "pos": [0, 0, 0]}, {"t": 0.22, "pos": [0, 0, 0], "ease": "linear"},
                             {"t": 0.28, "pos": [0, -0.004, 0], "ease": "out"}, {"t": 0.45, "pos": [0, 0.006, 0], "ease": "out"},
                             {"t": 0.6, "pos": [0, 0, 0], "ease": "in"}])
for i, item in enumerate(breakfast()):
    d = 0.02 + 0.012 * (i % 4)
    s = SPINS[i]
    ks = [{"t": 0, "pos": [0, 0.7 + 0.08 * (i % 3), 0], "rot": [s[0] * 0.3, s[1] * 0.3, s[2] * 0.3], "ease": "in"},
          {"t": 0.24 + d * 0.5, "pos": [0, 0, 0], "rot": [0, 0, 0], "ease": "linear"},
          {"t": 0.32 + d, "pos": [0, 0.018, 0], "rot": [0, 0, 0], "ease": "out"},
          {"t": 0.42 + d, "pos": [0, 0, 0], "rot": [0, 0, 0], "ease": "in"}]
    o += with_keys(item, ks)
scenes.append(scene(2.0, o, cam({"t": 0, "pos": [-0.42, 0.26, 0.42], "target": [0, 0.03, 0], "focal": 45, "fstop": 4},
                                 {"t": 0.3, "pos": [-0.42, 0.26, 0.42], "target": [0, 0.05, 0], "focal": 45, "fstop": 4},
                                 {"t": 2, "pos": [-0.34, 0.22, 0.34], "target": [0, 0.03, 0], "focal": 45, "fstop": 4, "ease": "inOut"}),
                    ))

# S8 16-20: FERRO letters tossed out of the pan onto the tile.
PZ = -0.16
o = with_keys(pan_parts(), pan_jolt([0.2], 0.02))
for p in o: p["pos"] = [p.get("pos", [0, 0, 0])[0], p.get("pos", [0, 0, 0])[1], p.get("pos", [0, 0, 0])[2] + PZ]
o += with_keys(breakfast()[0], [{"t": 0, "pos": [0, 0, PZ]}])
LET = "FERRO"
for i, ch in enumerate(LET):
    fx, fz = (i - 2) * 0.088, 0.13
    t0, T = 0.2 + 0.09 * i, 0.95 + 0.05 * i
    h = 0.34 + 0.03 * (i % 2)
    ks = [{"t": 0, "pos": [-fx, 0.03 - 0.0, PZ - fz], "rot": [90, 0, 0], "ease": "linear"},
          {"t": t0, "pos": [-fx, 0.03, PZ - fz], "rot": [90, 0, 0], "ease": "linear"}]
    n = 36
    for j in range(1, n + 1):
        u = j / n
        y = 0.03 * (1 - u) + 4 * h * u * (1 - u)
        ks.append({"t": round(t0 + u * T, 4), "pos": [round(-fx * (1 - u), 4), round(y, 4), round((PZ - fz) * (1 - u), 4)],
                   "rot": [round(90 - 450 * u, 1), round(40 * (1 - u) * (1 if i % 2 else -1), 1), 0], "ease": "linear"})
    ks += [{"t": t0 + T + 0.07, "pos": [0, 0.008, 0], "rot": [0, 0, 0], "ease": "out"},
           {"t": t0 + T + 0.14, "pos": [0, 0, 0], "rot": [0, 0, 0], "ease": "in"}]
    o.append({"type": "text", "name": f"L{i}", "text": ch, "font": "Archivo", "weight": 900, "height": 0.08,
              "depth": 0.016, "bevel": 0.002, "pos": [fx, 0.0, fz], "rot": [-90, 0, 0], "material": TXT, "keys": ks})
cta = {"text": "Back it on Kickstarter", "font": "Archivo", "weight": 900, "size": "5.6vh", "x": "50%", "y": "88%",
       "anchor": "center", "color": "#FFF4E6", "shadow": {"x": 0, "y": 3, "blur": 14, "color": "#2A0E04CC"}, "start": 2.0, "enter": {"preset": "fadeUp", "duration": 0.5}}
scenes.append(scene(4.0, o, cam({"t": 0, "pos": [0.0, 0.62, 0.62], "target": [0, 0.04, 0.0], "focal": 40, "fstop": 5.6, "focus": "L2"},
                                 {"t": 4, "pos": [0.0, 0.52, 0.5], "target": [0, 0.02, 0.03], "focal": 40, "fstop": 5.6, "focus": "L2", "ease": "inOut"}),
                    layers=[cta], walls=False))

spec = {
    "size": "1:1", "fps": 30,
    "music": {"style": "acoustic", "instrumentation": "band", "bpm": 120, "key": "D",
              "sections": [{"name": "intro", "bars": 2, "energy": 0.5}, {"name": "build", "bars": 4, "energy": 0.8},
                           {"name": "lift", "bars": 1, "energy": 0.9}, {"name": "hang", "bars": 0.75, "energy": 0.1},
                           {"name": "payoff", "bars": 2.25, "energy": 1, "stop": 0}],
              "hits": ["@bar8"], "end": "ring", "followCuts": False},
    "sound": "auto",
    "post": {"grain": 0.04, "vignette": 0.15},
    "scenes": scenes,
}
json.dump(spec, open(sys.argv[1] if len(sys.argv) > 1 else "spec.json", "w"), indent=1)
