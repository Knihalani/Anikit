#!/usr/bin/env python3
"""Railyard launch film: sympathetic tuning forks.
Writes ../spec.json (and frames spec with --frames). One world: satin-steel forks
on oak resonator boxes, a pale plaster table, north-window daylight."""
import json, math, sys, os

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..")

BPM = 96
BEAT = 60 / BPM          # 0.625 s
BAR = 4 * BEAT           # 2.5 s

STEEL = {"extends": "chrome", "base": "#C9CED3", "roughness": 0.28, "imperfections": 0.45}
OAK = {"texture": "polyhaven:texture/oak_veneer_01", "scale": 3, "imperfections": 0.3}
HOLE = {"base": "#17120D", "roughness": 0.95}
FELT = {"base": "#C7402A", "roughness": 0.95}          # the one accent: the mallet
HANDLE = {"base": "#2B2420", "roughness": 0.5}

R_BEND = 0.0145
U = []
for k in range(25):
    a = math.pi * k / 24
    U.append([round(-R_BEND * math.cos(a), 5), round(-R_BEND * math.sin(a), 5), 0])


def ring_env(strikes, damp_at=None, amp=2.6, k=0.55):
    """Expression for a tine's rotation (deg): fast vibration, decaying after each strike.
    The frequency is high on purpose: motion blur smears the tine into a ghost."""
    terms = []
    for ts in strikes:
        terms.append(f"clamp((t-{ts:.4f})*400,0,1)*exp(-{k}*max(t-{ts:.4f},0))")
    env = terms[0]
    for tm in terms[1:]:
        env = f"max({env},{tm})"
    if damp_at is not None:
        env = f"({env})*(1-clamp((t-{damp_at:.4f})*12,0,1))"
    return f"{amp}*sin(t*2764.6)*{env}"


def fork(name, x, z, strikes, damp_at=None, ry=0.0, amp=2.6, box=True, brand=None):
    """A fork on its resonator box at (x, z). Tines ring after each strike time."""
    objs = []
    top = 0.12  # box height
    if box:
        objs += [
            {"type": "box", "name": f"{name}_box", "size": [0.34, top, 0.16], "pos": [x, top / 2, z], "rot": [0, ry, 0], "material": OAK},
        ]
        # sound hole on the open end (+x side, rotated with box)
        hx = x + 0.1705 * math.cos(math.radians(ry))
        hz = z - 0.1705 * math.sin(math.radians(ry))
        objs.append({"type": "box", "size": [0.002, 0.085, 0.12], "pos": [hx, top / 2, hz], "rot": [0, ry, 0], "material": HOLE})
    base = top if box else 0
    stem_h = 0.06
    objs += [
        {"type": "cylinder", "radius": 0.012, "height": 0.012, "pos": [x, base + 0.006, z], "material": STEEL},
        {"type": "cylinder", "radius": 0.0055, "height": stem_h, "pos": [x, base + stem_h / 2, z], "material": STEEL},
        {"type": "tube", "points": U, "radius": 0.0042, "pos": [x, base + stem_h + R_BEND, z], "rot": [0, ry, 0], "material": STEEL},
    ]
    tine_len = 0.16
    cy = base + stem_h + R_BEND + tine_len / 2
    for side, s in (("L", -1), ("R", 1)):
        rz = ring_env(strikes, damp_at, amp) if strikes else "0"
        # rotate about the tine's centre; tips splay outwards in opposite directions
        expr = f"={-s}*({rz[:]})" if strikes else 0
        dx = s * R_BEND * math.cos(math.radians(ry))
        dz = -s * R_BEND * math.sin(math.radians(ry))
        objs.append({"type": "cylinder", "name": f"{name}_tine{side}", "radius": 0.0042, "height": tine_len,
                     "pos": [round(x + dx, 5), round(cy, 5), round(z + dz, 5)],
                     "rot": [0, ry, expr], "material": STEEL})
    if brand:
        objs.append(brand)
    return objs


def mallet(x_tine, y, z, ts_list, side=-1, travel=0.16):
    """Felt mallet that swings in along x, hits the tine at each ts, and rebounds."""
    # distance from contact: min over strikes of a parabola-ish curve
    terms = [f"min(abs(t-{ts:.4f})*3.2,1)" for ts in ts_list]
    d = terms[0]
    for tm in terms[1:]:
        d = f"min({d},{tm})"
    head_r = 0.016
    xc = x_tine + side * (0.0042 + head_r)
    xe = f"={xc:.5f}{'+' if side > 0 else '-'}{travel}*pow({d},1.6)"
    return [
        {"type": "sphere", "name": "mallet", "radius": head_r, "pos": [xe, y, z], "material": FELT},
        {"type": "cylinder", "radius": 0.004, "height": 0.26, "pos": [xe.replace(f"={xc:.5f}", f"={xc + side*0.13:.5f}"), y, z], "rot": [0, 0, 90], "material": HANDLE},
    ]


def scene3d(objects, cam_keys, rig=None, extra=None, set_color="#C3C6C0"):
    props = {
        "env": {"hdri": "polyhaven:hdri/blue_photo_studio", "background": False, "exposure": -0.3},
        "rig": rig or {"preset": "window", "gobo": "blinds", "warmth": -0.15, "ratio": 4},
        "set": {"preset": "sweep", "color": set_color},
        "camera": {"shake": "breath", "keys": cam_keys},
        "objects": objects,
    }
    if extra:
        props.update(extra)
    return {"component": "scene3d", "props": props}


def key(t, pos, target, focus=None, fstop=2.8, focal=None, shutter=0.5):
    k = {"t": t, "pos": pos, "target": target, "fstop": fstop, "shutter": shutter}
    if focus:
        k["focus"] = focus
    if focal:
        k["focal"] = focal
    return k


# ---------------------------------------------------------------- shots
def shot1():
    """Macro, low: the felt mallet already swinging in; strike on beat 2. Hook."""
    d = BAR
    ts = [BEAT * 1]
    objs = fork("a", 0, 0, ts) + mallet(-R_BEND, 0.12 + 0.06 + R_BEND + 0.12, 0, ts)
    cam = [key(0, [0.16, 0.25, 0.50], [0, 0.27, 0], "a_tineL", 2.8, 70),
           key(d, [0.11, 0.26, 0.42], [0, 0.275, 0], "a_tineL", 2.8, 70)]
    return d, [scene3d(objs, cam)]


def shot2():
    """Long table: fork A struck in the foreground (soft), fork B far back rings at once. Rack focus."""
    d = BAR
    ts = [BEAT * 1]
    objs = fork("a", -0.25, 0.9, ts) + mallet(-0.25 - R_BEND, 0.12 + 0.06 + R_BEND + 0.12, 0.9, ts)
    objs += fork("b", 0.22, -0.7, ts, ry=-25)
    cam = [key(0, [-0.55, 0.36, 1.75], [0.1, 0.28, -0.6], "a_tineL", 2.0, 85),
           key(BEAT * 1.3, [-0.55, 0.36, 1.72], [0.1, 0.28, -0.6], "a_tineL", 2.0, 85),
           key(BEAT * 2.4, [-0.54, 0.36, 1.68], [0.1, 0.24, -0.6], "b_tineL", 2.8, 85),
           key(d, [-0.53, 0.36, 1.64], [0.1, 0.24, -0.6], "b_tineL", 2.8, 85)]
    return d, [scene3d(objs, cam)]


def shot3():
    """Top-down: a very long pale table, one box at each far end. One strike, both ring."""
    d = BAR
    ts = [BEAT * 1]
    objs = fork("a", -1.15, 0, ts, ry=0, amp=4) + mallet(-1.15 - R_BEND, 0.12 + 0.06 + R_BEND + 0.12, 0, ts)
    objs += fork("b", 1.15, 0.05, ts, ry=180, amp=4)
    cam = [key(0, [0, 2.9, 0.02], [0, 0, 0], None, 8, 35),
           key(d, [0, 2.75, 0.02], [0, 0, 0], None, 8, 35)]
    rig = {"preset": "window", "gobo": "window", "warmth": -0.1, "ratio": 3}
    return d, [scene3d(objs, cam, rig=rig, extra={"set": {"preset": "sweep", "color": "#CFCFC8"}})]


def shot4():
    """Scale: a hall-floor field of forks; one strike, all of them ring together. Low dolly."""
    d = BAR
    ts = [BEAT * 1]
    objs = []
    n = 0
    for r in range(5):
        for c in range(7):
            x = -1.5 + c * 0.5 + (0.25 if r % 2 else 0)
            z = 0.6 - r * 0.55
            objs += fork(f"f{n}", round(x, 3), round(z, 3), ts, ry=0, amp=2.6)
            n += 1
    objs += mallet(-1.5 - R_BEND, 0.12 + 0.06 + R_BEND + 0.12, 0.6, ts)
    cam = [key(0, [-1.9, 0.5, 1.7], [-0.1, 0.12, -0.9], "f0_tineL", 4, 35),
           key(d, [-1.5, 0.52, 1.75], [0.15, 0.12, -0.9], "f0_tineL", 4, 35)]
    return d, [scene3d(objs, cam, rig={"preset": "window", "gobo": "blinds", "warmth": 0.0, "ratio": 4})]


def shot5():
    """Hard stop: macro, the tines hang dead still. Silence."""
    d = BAR / 2
    objs = fork("b", 0, 0, [])
    cam = [key(0, [0.16, 0.30, 0.42], [0, 0.27, 0], "b_tineL", 4, 70, 0.2),
           key(d, [0.15, 0.30, 0.40], [0, 0.27, 0], "b_tineL", 4, 70, 0.2)]
    return d, [scene3d(objs, cam, rig={"preset": "window", "gobo": "blinds", "warmth": 0.05, "ratio": 6})]


def shot6():
    """Payoff: the last strike; pull back as the window light warms onto the box front:
    the name is branded into the oak."""
    d = BAR * 1.5
    ts = [0.0001]
    brand = {"type": "plane", "size": [0.33, 0.11], "pos": [0, 0.058, 0.0805], "rot": [0, 0, 0],
             "texture": "#brand", "material": "decal", "thin": True, "shadow": False}
    objs = fork("b", 0, 0, ts, amp=2.2, brand=brand)
    cam = [key(0, [0.04, 0.31, 0.32], [0, 0.29, 0], "b_tineL", 2.2, 70),
           key(1.7, [0.02, 0.13, 0.66], [0, 0.11, 0], "b_box", 5.6, 50),
           key(d, [0.015, 0.128, 0.62], [0, 0.105, 0], "b_box", 5.6, 50)]
    rig = {"preset": "window", "gobo": "blinds", "warmth": 0.35, "ratio": 4}
    return d, [scene3d(objs, cam, rig=rig)]


def build():
    shots = [shot1, shot2, shot3, shot4, shot5, shot6]
    scenes = []
    for fn in shots:
        d, layers = fn()
        scenes.append({"duration": round(d, 4), "background": "#C3C6C0", "layers": layers})
    spec = {
        "size": "16:9",
        "fps": 30,
        "intent": {"pace": "medium"},
        "fonts": {},
        "compositions": {
            "brand": {"size": [1200, 400], "duration": 4, "layers": [
                {"text": "Railyard", "font": "Archivo", "weight": 800, "axes": {"wdth": 118}, "size": 200, "x": 600, "y": 160, "color": "#C7402A", "anchor": "center", "opacity": 0.95},
                {"text": "One API call. Settled instantly.", "font": "Archivo", "weight": 700, "axes": {"wdth": 100}, "size": 64, "x": 600, "y": 318, "color": "#C7402A", "anchor": "center", "opacity": 0.95}
            ]}
        },
        "music": music(),
        "sound": "auto",
        "post": {"grain": 0.12, "vignette": 0.15},
        "scenes": scenes,
    }
    return spec


def music():
    # strike times (global seconds) for the fork voice
    return {
        "bpm": BPM, "key": "A", "end": "ring",
        "define": {"fork": {"synth": {"osc": [{"wave": "sine", "level": 0.8}, {"wave": "sine", "semis": 31, "level": 0.06}],
                                       "amp": [0.002, 4.0, 0.0, 2.5]}}},
        "tracks": [
            # the forks: one A per strike (shot strikes on beat 2 of bars 1-4, payoff on bar 5 beat 3)
            {"inst": "fork", "notes": "o5 r4 A/2.! | r4 A/2.! | r4 [A E']/2.! | r4 [A E' A']/2.! | r2 [A, E A C#']/2! | _1", "gain": -2, "reverb": 0.35},
            # body of the boxes: cello drone that grows
            {"inst": "cello", "notes": "o2 r1 | A/1 | A/1 | A/2 E/2 | r2 r2 | A/1", "gain": -9, "reverb": 0.4},
            # felt strike + box knock
            {"inst": "drums", "drums": {"tom": "....x...........", "rim": "........o......."}, "bars": 4, "gain": -10},
            {"inst": "vibraphone", "notes": "o5 r1 | r1 | E/8 A/8 r4 r2 | E/8 A/8 C#'/8 E'/8 r2 | r1 | r2 E'/2", "gain": -12, "reverb": 0.5},
        ],
        "master": {"lufs": -15, "reverb": "hall"},
    }


if __name__ == "__main__":
    spec = build()
    with open(os.path.join(OUT, "spec.json"), "w") as f:
        json.dump(spec, f, indent=1)
    print("wrote spec.json", sum(s["duration"] for s in spec["scenes"]), "s")
