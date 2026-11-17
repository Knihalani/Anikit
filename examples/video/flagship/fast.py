"""EVERY FRAME, fast cut: a <= 15 s test at 130 BPM, one hard cut every two beats.

The same world as the long film (glass frames in a white studio), cut for speed and
scale: every shot shows something a browser tool can't render: tens of thousands of
ray-traced glass panes, each carrying its own playing video frame.

    python3 examples/video/flagship/fast.py [--proxy] [--draft]   # writes fast.json
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE))
import films as F  # noqa: E402

BPM = 130
BEAT = 60 / BPM
SHOT = 2 * BEAT            # 0.923 s
PANE = [1.6, 0.9, 0.008]
FILMS = ["fold", "fold2", "fold3", "fold4"]   # four shader films (Fold variants for now)


def pane_tex(film, frame="=i"):
    return {"composition": film, "frame": frame, "face": "front"}


def cam(keys, **kw):
    out = {"interp": "smooth", "keys": keys}
    out.update(kw)
    return out


def key(t, pos, target, focal=35, fstop=2.8, shutter=0.5, **kw):
    d = {"t": round(t, 4), "pos": pos, "target": target, "focal": focal, "fstop": fstop, "shutter": shutter}
    d.update(kw)
    return d


def sm(u):
    return f"({u})*({u})*(3-2*({u}))"


def shot(dur, camera, objects, **props):
    p = {"env": {"preset": "cyc", "color": "#EEEBE5", "haze": 0.0012}, "camera": camera, "objects": objects,
         "quality": QUALITY, "bloom": 0.08}
    p.update(props)
    return {"duration": round(dur, 4), "transition": "cut",
            "layers": [{"component": "scene3d", "props": p}]}


# --- the shots -------------------------------------------------------------------------------

def s_macro():
    """One pane, very close: the picture inside the glass, light raking across the edge."""
    d = SHOT
    return shot(d, cam([key(0, [-1.15, 0.78, 1.55], [0.25, 0.45, 0], 50, 2.0),
                        key(d, [-0.95, 0.7, 1.4], [0.3, 0.45, 0], 50, 2.0)]),
                [{"type": "slab", "size": PANE, "radius": 0.03, "material": "pane", "pos": [0, 0.47, 0],
                  "texture": pane_tex(FILMS[1], "=t*60 + 300")}])


def s_deck():
    """SNAP: 1,800 panes fan out from one stack into a full circle in a beat."""
    d = SHOT
    u = f"clamp(t/{BEAT * 1.2},0,1)"
    fan = sm(u)
    return shot(d, cam([key(0, [2.6, 2.2, 3.0], [0, 0.4, 0], 28, 3.6), key(d, [3.2, 2.8, 3.8], [0, 0.3, 0], 28, 4.4)]),
                [{"type": "slab", "count": 120, "size": PANE, "radius": 0.03, "material": "pane",
                  "pos": f"=[0.8*cos(i*3*{fan}*0.0174533), 0.47, -0.8*sin(i*3*{fan}*0.0174533)]",
                  "rot": f"=[0, i*3*{fan}, 0]",
                  "texture": pane_tex(FILMS[0])}])


def s_highway(whip=True):
    """Whip into the highway: four strips racing to the horizon, machine streaking off."""
    d = SHOT
    objs = []
    for k in range(4):
        objs.append({"type": "slab", "count": 1800, "size": PANE, "radius": 0.03, "material": "pane",
                     "pos": f"=[1.7*i, 0.47, {-4 * k}]", "texture": pane_tex(FILMS[k])})
        objs.append({"type": "box", "count": 1800, "size": [1.7, 0.003, 0.08], "material": "rail", "shadow": False,
                     "pos": f"=[1.7*i, 0.0015, {-4 * k}]"})
    k0 = key(0, [-4, 2.4, 2.0], [10, 0.2, 6] if whip else [40, 0, -6], 30, 4, 1.0)
    k1 = key(0.18, [-3, 2.3, 1.0], [40, 0, -6], 30, 4, 1.0)
    k2 = key(d, [30, 1.4, 0.5], [90, 0, -6], 30, 4, 0.8)
    return shot(d, cam([k0, k1, k2]), objs)


def s_vortex():
    """Crash zoom down into a vortex of 40,000 panes, each a different frame."""
    d = SHOT
    n = 40000
    r = "(2+sqrt(i)*0.22)"
    a = f"(i*0.0390 + t*{0.6})"
    objs = [{"type": "slab", "count": n, "size": PANE, "radius": 0.03, "material": "pane",
             "pos": f"=[{r}*cos({a}), 0.47 + 0.0*i, {r}*sin({a})]",
             "rot": f"=[0, -{a}*57.2958 + 90, 0]", "texture": pane_tex(FILMS[2])}]
    return shot(d, cam([key(0, [0.1, 70, 0.1], [0, 0, 0], 35, 8, 0.8), key(d, [0.6, 9, 1.2], [0, 0, 0], 35, 8, 0.8)],
                       ease=False), objs)


def s_run(lane, dur):
    """The locked run: one pane per frame of camera travel, the film plays inside the glass."""
    pitch, fps = 1.7, 60
    v = pitch * fps
    x0 = 300.0 + lane * 50
    objs = [{"type": "slab", "count": 1800, "size": PANE, "radius": 0.03, "material": "pane",
             "pos": f"=[1.7*i, 0.47, {-4 * k}]", "texture": pane_tex(FILMS[k])} for k in range(4)]
    z = -4 * lane + 3.6
    keys = [key(0, [x0, 0.72, z], [x0 + 5, 0.3, z - 3.6], 50, 2.8, 0.0),
            key(dur, [x0 + v * dur, 0.72, z], [x0 + v * dur + 5, 0.3, z - 3.6], 50, 2.8, 0.0)]
    return shot(dur, cam(keys, interp="linear", ease=False), objs)


def s_dominoes():
    """A 10,000-pane domino wave sweeping away from camera."""
    d = SHOT
    rows, cols = 100, 100
    delay = f"(floor(i/{cols})*0.009 + abs((i - floor(i/{cols})*{cols}) - {cols / 2})*0.002)"
    fall = sm(f"clamp((t - {delay})/0.28, 0, 1)")
    objs = [{"type": "slab", "count": rows * cols, "size": [1.6, 0.9, 0.012], "radius": 0.03, "material": "pane",
             "pos": f"=[(i - floor(i/{cols})*{cols} - {cols / 2})*1.75, 0.47, -floor(i/{cols})*0.62]",
             "rot": f"=[-84*{fall}, 0, 0]", "texture": pane_tex(FILMS[3])}]
    return shot(d, cam([key(0, [0, 1.2, 3.2], [0, 0.2, -12], 28, 4, 0.6), key(d, [0, 1.6, 2.4], [0, 0, -16], 28, 6, 0.6)]),
                objs)


def s_crush():
    """The strip folds into a stack and crushes into a tile."""
    d = SHOT
    u = sm(f"clamp(t/{BEAT * 1.1},0,1)")
    c = sm(f"clamp((t-{BEAT * 1.1})/{BEAT * 0.8},0,1)")
    x = f"(1.7*i*(1-{u}) + i*0.012*{u})*(1-0.95*{c})"
    objs = [{"type": "slab", "count": 600, "size": PANE, "radius": 0.03, "material": "pane",
             "pos": f"=[{x}, 0.47, 0]", "rot": f"=[0, 90*{u}, 0]", "texture": pane_tex(FILMS[1])}]
    return shot(d, cam([key(0, [-2.8, 1.0, 3.2], [3, 0.4, 0], 35, 3.5), key(d, [-1.6, 0.7, 1.8], [0.2, 0.4, 0], 50, 2.2)]),
                objs)


def s_tunnel():
    """Speed-ramped pull back through a tunnel of 24,000 panes."""
    d = SHOT * 1.5
    per = 24
    ang = f"((i - floor(i/{per})*{per})*{360 / per})"
    z = f"(-floor(i/{per})*0.9)"
    objs = [{"type": "slab", "count": 24000, "size": PANE, "radius": 0.03, "material": "pane",
             "pos": f"=[3.2*cos({ang}*0.0174533), 3.2 + 3.2*sin({ang}*0.0174533), {z}]",
             "rot": f"=[0, 0, {ang} + 90]", "texture": pane_tex(FILMS[(2) % 4], "=i + t*60")}]
    return shot(d, cam([key(0, [0, 3.2, -300], [0, 3.2, -400], 24, 8, 1.0), key(d * 0.35, [0, 3.2, -120], [0, 3.2, -220], 24, 8, 1.0),
                        key(d, [0, 3.2, 6], [0, 3.2, -100], 24, 8, 0.4)]), objs, floor={"reflect": 0.0})


def s_logo(dur):
    """Panes stream past; chrome ANIKIT VIDEO stands on the floor."""
    rise = sm(f"clamp(t/{BEAT * 1.5},0,1)")
    objs = [
        {"text": "ANIKIT VIDEO", "height": 0.42, "depth": 0.2, "bevel": 0.03, "material": "chrome",
         "font": "Inter Display", "weight": 800, "pos": f"=[0, -0.75*(1-{rise}), 0]"},
        {"text": "Words in. Film out.", "height": 0.16, "depth": 0.02, "bevel": 0.004, "material": "ink",
         "font": "Inter Display", "weight": 600, "pos": f"=[0, -0.2*(1-{rise}), 0.9]"},
        {"type": "slab", "count": 400, "size": PANE, "radius": 0.03, "material": "pane",
         "pos": f"=[1.7*i - 340 + 300*{sm(f'clamp(t/{BEAT * 2},0,1)')}, 0.47, -3.5]", "texture": pane_tex(FILMS[0])},
    ]
    return shot(dur, cam([key(0, [-1.2, 1.1, 7.6], [0, 0.45, 0], 40, 7.0), key(dur, [-0.3, 1.0, 6.8], [0, 0.45, 0], 40, 6.4)]),
                objs)


QUALITY = "standard"


def build(proxy=False):
    global QUALITY
    if "--draft" in sys.argv:
        QUALITY = "draft"
    scenes = [s_macro(), s_deck(), s_highway(), s_vortex(),
              s_run(0, SHOT), s_run(1, SHOT), s_run(2, SHOT),
              s_dominoes(), s_crush(), s_tunnel(), s_logo(SHOT * 2.5)]
    total = sum(s["duration"] for s in scenes)
    comps = {}
    hues = ["#2A6F5E", "#6B2A7A", "#1F4E8C", "#B8862B"]
    for k, fid in enumerate(FILMS):
        if proxy:
            c = hues[k]
            comps[fid] = {"size": [480, 270], "duration": 30, "layers": [
                F.rect(240, 135, 456, 246, f"linear-gradient(135deg, {c}, #0B0B0B)", radius=4,
                       rotation=f"=sin(t*2.0 + {k})*4"),
                {"shape": "circle", "x": "=240 + 140*sin(t*1.7)", "y": "=135 + 60*cos(t*1.3)", "width": 70, "height": 70,
                 "fill": "#ffffffcc"}]}
        else:
            fc = F.fold()
            comps[fid] = fc
    spec = {
        "size": [1920, 1080], "fps": 60, "background": "#EEEBE5",
        "compositions": comps,
        "materials_note": "pane/rail/ink are per-scene materials",
        "scenes": scenes,
    }
    spec.pop("materials_note")
    mats = {"pane": {"extends": "glass", "base": "#FFFFFF", "roughness": 0.03, "tint": "#9FD8C8"},
            "rail": {"base": "#FF5A1F", "emissive": "#FF5A1F", "emissiveStrength": 1.5, "roughness": 0.5},
            "ink": {"base": "#15171A", "metallic": 0, "roughness": 0.45}}
    for s in scenes:
        s["layers"][0]["props"]["materials"] = mats
    out = HERE / ("fast-proxy.json" if proxy else "fast.json")
    out.write_text(json.dumps(spec, indent=0))
    print(f"{out}: {total:.2f} s, {len(scenes)} shots")


if __name__ == "__main__":
    build(proxy="--proxy" in sys.argv)
