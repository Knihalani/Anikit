"""EVERY FRAME: build the anikit-video spec from world.py + films.py.

    python3 examples/video/flagship/build.py            # writes spec.json
    target/release/anikit-video render examples/video/flagship/spec.json -o every-frame.mp4

The whole film is one scene with one `scene3d` layer: a camera track sampled from
world.py, the studio objects, six instanced strips of glass panes textured with the
six film compositions, and the 3D type. The score is music.mp3 (music.py).
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path

HERE = Path(__file__).parent
sys.path.insert(0, str(HERE))
import films as F  # noqa: E402
import world as Wd  # noqa: E402

T = Wd.T
KEY_HZ = 30          # object keys; the camera gets one key per output frame


def r3(v, n=4):
    return [round(float(c), n) for c in v]


# --- expressions shared by the strip instances -----------------------------------------

def sm(u):
    """Quintic smoother of an expression u (already clamped to 0..1)."""
    return f"({u})*({u})*({u})*(({u})*(6*({u})-15)+10)"


def seg(a, b):
    return f"clamp((t-({a}))/(({b})-({a})),0,1)"


def step(a):
    """0 before a, 1 after (sharp)."""
    return f"clamp((t-({a}))*1000,0,1)"


def emitted_expr():
    """n(t) of world.emitted, as an expression."""
    f1 = sm(seg(T["frame1_0"], T["frame1_1"]))
    s0 = T["stream0"]
    k = f"((t-{s0})/{Wd.BEAT})"
    beats = f"(floor({k})+{sm(f'clamp(({k}-floor({k}))/0.6,0,1)')})"
    u = seg(s0 + 1.5, T["stream1"])
    torrent = f"(4+{Wd.FRAMES - 4}*pow({u},2.6))"
    # phases: [frame1_0, stream0) -> f1 ; [stream0, stream0+1.5) -> 1 + beats ; then torrent
    return (f"({f1})*(1-{step(s0)})"
            f"+(1+min({beats},3))*({step(s0)}-{step(s0 + 1.5)})"
            f"+{torrent}*{step(s0 + 1.5)}")


def strip_instances(k, fid):
    """Lane k's 1,800 panes. Visible once laid; fold into a stack at 50.6-52; crush 52.5-54; gone at 54.5."""
    xc, jc = Wd.fold_centre(), Wd.fold_jc()
    n = emitted_expr()
    x0 = f"({Wd.PITCH}*i-0.15)"
    xs = f"({xc}-(i-{jc})*{Wd.STACK_STEP})"
    delay = f"(0.3*(1-exp(-abs(i-{jc})/300)))"
    u = sm(f"clamp((t-({T['fold0']}+{delay}))/max({T['fold1']}-({T['fold0']}+{delay}),0.05),0,1)")
    yaw = "90*" + sm(f"clamp((t-({T['fold0']}+0.5*{delay}))/(0.5*{delay}+0.5),0,1)")
    c = sm(seg(T["crush0"], T["crush1"]))
    shrink = Wd.PANE_H * 0 + 0.25 / (Wd.FRAMES * Wd.STACK_STEP)
    x = f"({xc}+(({x0})+(({xs})-({x0}))*{u}-{xc})*(1+({shrink}-1)*{c}))"
    vis = f"clamp(({n})-i,0,1)*(1-{step(T['shrink0'])})"
    return {
        "type": "instances", "count": Wd.FRAMES,
        "shape": {"type": "slab", "size": [Wd.PANE_W, Wd.PANE_H, Wd.PANE_T], "radius": 0.03},
        "material": "pane",
        "pos": f"=[{x}, {Wd.PANE_Y}, {Wd.lane_z(k)}]",
        "rot": f"=[0, {yaw}, 0]",
        "scale": f"={vis}",
        "texture": {"composition": fid, "frame": "=i", "face": "front"},
    }


# --- keyed objects ------------------------------------------------------------------------

def sample_keys(fn, t0, t1, hz=KEY_HZ):
    keys = []
    n = max(1, int(round((t1 - t0) * hz)))
    for s in range(n + 1):
        t = t0 + (t1 - t0) * s / n
        keys.append(dict(t=round(t, 4), **fn(t)))
    return keys


def request_obj(k):
    def at(t):
        pos, yaw, vis = Wd.request_pose(k, t)
        return {"pos": r3(pos), "rot": [0, round(yaw, 2), 0], "scale": 1 if vis else 0}
    return {"type": "slab", "size": [Wd.PANE_W, Wd.PANE_H, Wd.PANE_T], "radius": 0.03, "material": "pane",
            "texture": [{"composition": f"req{k}", "face": "front"}, {"composition": f"plan{k}", "face": "back"}],
            "keys": sample_keys(at, 0.0, Wd.drop_time(k) + 0.1)}


def machine_objs():
    M = Wd.MACHINE
    depth, hgt = M["x1"] - M["x0"], M["y1"]
    zc, zw = (M["z0"] + M["z1"]) / 2, M["z1"] - M["z0"]

    def at(dx, y, dz=0.0):
        return lambda t: {"pos": r3((Wd.machine_x(Wd.emitted(t)) + dx, y, zc + dz))}
    t1 = T["stream1"] + 0.3
    objs = [{"type": "box", "size": [depth, hgt, zw], "radius": 0.03, "material": "aluminium-bead",
             "keys": sample_keys(at(0, hgt / 2), 0.0, t1)}]
    for k in range(Wd.LANES):
        z = Wd.lane_z(k)
        objs.append({"type": "box", "size": [Wd.PANE_W + 0.1, 0.004, 0.05], "material": "black",
                     "keys": sample_keys(lambda t, z=z: {"pos": r3((Wd.machine_x(Wd.emitted(t)), hgt + 0.002, z))}, 0.0, t1)})
        objs.append({"type": "box", "size": [0.05, 0.012, 0.05], "material": "led",
                     "keys": sample_keys(lambda t, z=z: {"pos": r3((Wd.machine_x(Wd.emitted(t)) + 0.7, hgt + 0.006, z + 0.3))},
                                         0.0, t1)})
    return objs


def rail_objs():
    out = []
    for k in range(Wd.LANES):
        z = Wd.lane_z(k)

        def at(t, z=z):
            n = Wd.emitted(t)
            x1 = Wd.machine_x(n) - 1.0 if t < T["stream1"] + 0.2 else Wd.pane_x(Wd.FRAMES - 1) + 0.8
            if t >= T["fold0"]:
                x1 = -1.0
            length = max(0.001, x1 + 1.0)
            return {"pos": r3((-1.0 + length / 2, 0.001, z)), "scale": [round(length, 3), 1, 1]}
        out.append({"type": "box", "size": [1.0, 0.002, 0.1], "material": "rail",
                    "keys": sample_keys(at, T["frame1_0"], T["fold0"] + 0.05, hz=15)})
    return out


def tile_screen_objs():
    """Each lane's crushed stack -> block -> tile -> rising screen that plays its film."""
    out = []
    xc = Wd.fold_centre()
    for k, f in enumerate(Wd.FILMS):
        def at(t, k=k):
            if t < T["rise0"]:
                bs = Wd.block_scale(t)
                pos = (xc, Wd.PANE_Y * bs + 0.01, Wd.block_z(k, t))
                # a block 0.25 thick in x, 0.9 high, 1.6 deep in z: the slab turned 90 degrees, thickened
                return {"pos": r3(pos), "rot": [0, 90, 0], "scale": [round(bs, 4), round(bs, 4), round(bs * 0.25 / 0.03, 4)]}
            pos, yaw, sc = Wd.screen_pose(k, t)
            thick = Wd.lerp(0.25 * 0.12 / 0.03, 1.0, Wd.smoother(Wd.seg(t, T["rise0"], T["rise0"] + 1.0)))
            return {"pos": r3(pos), "rot": [0, round(yaw, 2), 0], "scale": [round(sc, 4), round(sc, 4), round(sc * thick, 4)]}
        keys = [dict(t=0.0, pos=[xc, -5, 0], scale=[0, 0, 0]), dict(t=T["shrink0"] - 0.01, pos=[xc, -5, 0], scale=[0, 0, 0])]
        keys += sample_keys(at, T["shrink0"], Wd.DURATION)
        out.append({"type": "slab", "size": [Wd.PANE_W, Wd.PANE_H, 0.03], "radius": 0.03, "material": "pane",
                    "texture": {"composition": f["id"], "frame": f"=max(0, (t - {T['rise0'] + 1.0}) * {Wd.FPS})",
                                "face": "front"},
                    "keys": keys})
    return out


def caption_objs():
    out = []
    for c in Wd.CAPTIONS:
        if c["kind"] == "etched":
            continue
        pos = Wd.caption_anchor(c)
        logo = c["kind"] == "logo"
        h = c["h"]
        rise = 0.35 if not logo else T["logo1"] - T["logo0"]

        def at(t, c=c, pos=pos, logo=logo):
            u = Wd.smoother(Wd.seg(t, c["t0"], c["t0"] + (0.45 if not logo else rise)))
            v = 1 - Wd.smoother(Wd.seg(t, c["t1"] - 0.3, c["t1"])) if c["t1"] < Wd.DURATION else 1.0
            s = u * v
            y = pos[1] - (1 - u) * (c["h"] * 1.2)    # rises out of the floor
            return {"pos": r3((pos[0], y, pos[2])), "scale": round(max(s, 0.0), 4)}
        keys = [dict(t=0.0, pos=r3((pos[0], -2, pos[2])), scale=0)]
        keys += sample_keys(at, max(0.0, c["t0"] - 0.05), min(Wd.DURATION, c["t1"] + 0.05), hz=30)
        out.append({"type": "text", "text": c["text"], "height": h, "depth": h * (0.34 if logo else 0.06),
                    "bevel": h * (0.045 if logo else 0.01), "font": "Inter Display", "weight": 800 if logo else 650,
                    "material": "chrome" if logo else "ink", "align": "center", "rot": [0, c.get("yaw", 0), 0],
                    "keys": keys})
    return out


def camera():
    keys = []
    n = int(Wd.DURATION * Wd.FPS)
    for f in range(n + 1):
        t = f / Wd.FPS
        keys.append({"t": round(t, 5), "pos": r3(Wd.cam_pos(t), 5), "target": r3(Wd.cam_tgt(t), 5),
                     "focal": round(Wd.cam_focal(t), 2), "focus": round(Wd.cam_focus(t), 3),
                     "fstop": 2.8, "shutter": round(Wd.shutter(t) / 360, 3)})
    return {"interp": "linear", "keys": keys}


# --- 2D compositions for the requests and the plans -----------------------------------------

def request_comp(k):
    f = Wd.FILMS[k]
    return {"size": [1600, 900], "duration": 1, "background": "#F6F4EF", "layers": [
        F.txt("agent request", 120, 150, 30, "#15171A80", font="JetBrains Mono", weight=500, anchor="left"),
        F.rect(800, 470, 1360, 420, "#FFFFFF", radius=48, stroke={"color": "#15171A1a", "width": 2},
               shadow={"x": 0, "y": 18, "blur": 50, "color": "#15171A22"}),
        F.txt(f["request"], 800, 470, 58, "#15171A", font="Inter Display", weight=650, width=1200, lineHeight=1.22),
        {"shape": "circle", "x": 150, "y": 780, "width": 44, "height": 44, "fill": "#FF5A1F"},
        F.txt(f"client · {f['name']}", 196, 780, 28, "#15171A99", weight=600, anchor="left"),
    ]}


def plan_comp(k):
    f = Wd.FILMS[k]
    code = json.dumps({
        "size": "16:9", "fps": 60, "duration": 30,
        "theme": {"look": f["id"]},
        "scenes": ["… 14 scenes"],
        "audio": [{"src": "score.mp3"}],
    }, indent=2)
    code = code.replace('"… 14 scenes"', '{ "layers": [ … ] }')
    return {"size": [1600, 900], "duration": 2, "background": "#15171A", "layers": [
        {"component": "code", "width": 1600, "height": 900,
         "props": {"code": code, "language": "json", "title": f"{f['id']}.json", "fontSize": 34, "chrome": "minimal",
                   "background": "#15171A", "highlights": [{"lines": [l], "at": 0.12 * l, "until": 0.12 * l + 0.3}
                                                            for l in range(1, 12)]}},
    ]}


def build():
    # lanes whose film is not built yet show Fold (a 3D shader) so the outer world can be judged
    comps = {f["id"]: getattr(F, f["id"], F.fold)() for f in Wd.FILMS}
    for k in range(Wd.LANES):
        comps[f"req{k}"] = request_comp(k)
        comps[f"plan{k}"] = plan_comp(k)
    objects = []
    objects += [request_obj(k) for k in range(Wd.LANES)]
    objects += machine_objs()
    objects += rail_objs()
    objects += [strip_instances(k, f["id"]) for k, f in enumerate(Wd.FILMS)]
    objects += tile_screen_objs()
    objects += caption_objs()
    scene3d = {
        "env": {"preset": "cyc", "color": "#EEEBE5", "haze": 0.0006, "key": [-0.5, 1.0, 0.4]},
        "camera": camera(),
        "materials": {
            "pane": {"base": "#FFFFFF", "metallic": 0, "roughness": 0.04, "transmission": 0.15, "ior": 1.5,
                     "tint": "#9FD8C8"},
            "led": {"base": "#FF5A1F", "emissive": "#FF5A1F", "emissiveStrength": 6},
            "rail": {"base": "#FF5A1F", "emissive": "#FF5A1F", "emissiveStrength": 1.5, "roughness": 0.5},
            "ink": {"base": "#15171A", "metallic": 0, "roughness": 0.45},
        },
        "objects": objects,
        "quality": "standard",
    }
    spec = {
        "size": [Wd.W, Wd.H], "fps": Wd.FPS, "duration": Wd.DURATION, "background": "#EEEBE5",
        "compositions": comps,
        "scenes": [{"id": "every-frame", "duration": Wd.DURATION, "transition": "cut",
                    "layers": [{"component": "scene3d", "props": scene3d}]}],
        "audio": [{"src": "music.mp3", "volume": 0.9}],
    }
    out = HERE / "spec.json"
    out.write_text(json.dumps(spec, separators=(",", ":")))
    print(f"{out}: {out.stat().st_size / 1024:.0f} KB, {len(objects)} objects, {len(comps)} compositions")


if __name__ == "__main__":
    build()
