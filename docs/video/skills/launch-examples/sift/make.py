#!/usr/bin/env python3
"""Sift (fictional): 15 s, 16:9 launch. Concept #5 physical simulation. Writes sift.json."""
import json
B = 0.46875  # one beat at 128 BPM
INK, PAPER, SIG, MUTE = "#14161B", "#F2EFE8", "#FF4D2E", "#8A8F98"
FD, FM = "Inter Display", "JetBrains Mono"

rows = [("Maya Chen", "Needs you", "Approve the Q3 budget by Friday — one figure changed."),
        ("Ops alerts", "Muted", "12 alerts, all resolved overnight."),
        ("Daniel at Arcwell", "Needs you", "Can we move the call to 3 pm?"),
        ("Newsletters", "Later", "7 issues bundled into one digest."),
        ("Lena Ortiz", "FYI", "Shipped the new onboarding; no action needed.")]
def inbox_layers(focus=None):
    L = [{"shape": "rect", "width": 1200, "height": 760, "fill": PAPER, "x": 0, "y": 0, "anchor": "topLeft"},
         {"text": "Today", "font": FD, "size": 64, "weight": 800, "color": INK, "x": 60, "y": 50, "anchor": "topLeft"},
         {"text": "SORTED 06:58", "font": FM, "size": 22, "color": SIG, "x": 1140, "y": 78, "anchor": "right"}]
    for k, (who, tag, line) in enumerate(rows):
        y = 170 + k * 112
        dim = 0.25 if focus is not None and k != focus else 1
        L += [{"shape": "rect", "width": 1080, "height": 2, "fill": "#D9D4CA", "x": 60, "y": y - 14, "anchor": "topLeft"},
              {"text": who, "font": FD, "size": 34, "weight": 700, "color": INK, "x": 60, "y": y, "anchor": "topLeft", "opacity": dim},
              {"text": tag.upper(), "font": FM, "size": 20, "color": SIG if tag == "Needs you" else MUTE, "x": 1140, "y": y + 8, "anchor": "topRight", "opacity": dim},
              {"text": line, "font": "Inter", "size": 26, "color": "#4A4E57", "x": 60, "y": y + 48, "anchor": "topLeft", "opacity": dim}]
    return L

def cam(keys):
    return {"keys": [{"t": t, "pos": p, "target": g, "focal": f, "fstop": s} for t, p, g, f, s in keys]}

def s3d(props, **kw):
    base = {"quality": "draft", "resolution": 0.5, "bloom": 0.25, "lights": LIGHTS}
    base.update(props)
    return {"component": "scene3d", "props": base, **kw}

def slam(text, bg, fg, dur, x="50%", rot=-3, size="30vh"):
    return {"duration": dur, "transition": "cut", "background": bg,
            "camera": {"animate": [{"from": {"zoom": 1.18, "rotation": rot}, "to": {"zoom": 1, "rotation": 0}, "duration": 0.2, "ease": "outExpo"}],
                       "shake": {"amount": 7, "at": 0, "duration": 0.15}},
            "layers": [{"text": text, "font": FD, "size": size, "weight": 900, "color": fg, "letterSpacing": "-0.05em", "x": x, "y": "50%", "anchor": "center",
                        "animate": [{"from": {"scale": 1.9, "opacity": 0, "blur": 14}, "duration": 0.12, "ease": "outExpo"}]}]}

tile = {"type": "box", "size": [0.62, 0.4, 0.02], "material": {"base": PAPER, "roughness": 0.45}}
red_tile = {"type": "box", "size": [0.62, 0.4, 0.02], "material": {"base": SIG, "roughness": 0.35}}
env = {"preset": "studio", "color": "#1A1C22"}
LIGHTS = [{"dir": [-0.5, 0.8, 0.6], "size": [40, 12], "color": "#FFF4E8", "intensity": 5, "key": True}, {"dir": [0.7, 0.3, -0.6], "size": [8, 50], "color": "#FF6A4A", "intensity": 3}]

def w3(word, pos, h, mat, rot=(0, 0, 0)):
    """A hero word as 3D type in the set (snaps in on the beat)."""
    return {"text": word, "height": h, "depth": h * 0.3, "bevel": h * 0.03, "font": FD, "weight": 900, "material": mat, "pos": list(pos),
            "keys": [{"t": 0, "rot": [rot[0] - 30, rot[1], rot[2]], "scale": 1.3, "ease": "outExpo"}, {"t": 0.22, "rot": list(rot), "scale": 1}]}
WHITE = {"base": PAPER, "roughness": 0.35, "emissive": PAPER, "emissiveStrength": 0.45}
RED = {"base": SIG, "roughness": 0.3, "clearcoat": 1, "emissive": SIG, "emissiveStrength": 0.35}

scenes = []
# 1 Hook: envelopes raining in slow motion, one red one in focus.
scenes.append({"id": "hook", "duration": 2 * B, "layers": [s3d({
    "env": env, "camera": cam([(0, [0, 1.6, 4.2], [0, 1.4, 0], 50, 1.8), (2 * B, [0.2, 1.5, 3.7], [0, 1.3, 0], 52, 1.8)]),
    "objects": [dict(tile, count=180, pos="=[-3 + 6*rand(i), 3.4 - 3.6*fract(rand(i+5) + 0.12*t), -2.5 + 3*rand(i+9)]", rot="=[360*rand(i+1) + 30*t, 360*rand(i+2) + 20*t, 0]")] +
               [dict(red_tile, pos="=[0.1, 1.45 - 0.35*t, 0.6]", rot="=[18, 25 + 40*t, 8]")]})]})
# 2-3 Tension slams
def word3d(word, mat, env_, rot_from, cam_keys):
    """Type inside the world: an extruded word standing among falling tiles, snapped in on the beat."""
    return {"duration": 2 * B, "transition": "cut", "camera": {"shake": {"amount": 7, "at": 0, "duration": 0.15}}, "layers": [s3d({
        "env": env_, "camera": cam(cam_keys),
        "objects": [{"text": word, "height": 0.62, "depth": 0.22, "bevel": 0.02, "font": FD, "weight": 900, "material": mat,
                     "keys": [{"t": 0, "rot": [rot_from, -25, 0], "scale": 1.25, "ease": "outExpo"}, {"t": 0.22, "rot": [0, -8, 0], "scale": 1}]},
                    dict(tile, count=90, pos="=[-3 + 6*rand(i), 3.2 - 3.4*fract(rand(i+5) + 0.2*t), -2.6 + 1.9*rand(i+9)]",
                         rot="=[360*rand(i+1) + 40*t, 360*rand(i+2) + 30*t, 0]")]})]}
scenes.append(word3d("Too much", WHITE, env,
                     -30, [(0, [0.5, 0.6, 5.6], [0, 0.32, 0], 36, 3.2), (2 * B, [0.3, 0.55, 5.2], [0, 0.32, 0], 37, 3.2)]))
scenes.append(word3d("mail.", {"base": SIG, "roughness": 0.3, "clearcoat": 1}, {"preset": "cyc", "color": PAPER},
                     30, [(0, [-0.4, 0.55, 3.4], [0, 0.32, 0], 36, 2.4), (2 * B, [-0.2, 0.5, 3.1], [0, 0.32, 0], 38, 2.4)]))
# 4 Tension 3: the pile, blown apart in slow motion (riser peaks at the reveal)
scenes.append({"id": "pile", "duration": 2 * B, "transition": "cut", "layers": [s3d({
    "env": env, "camera": cam([(0, [0, 2.2, 5.5], [0, 0.4, 0], 35, 2.8), (2 * B, [0, 1.6, 4.4], [0, 0.4, 0], 38, 2.8)]),
    "physics": {"timeScale": "=1 - 0.8*smoothstep(0.35, 0.5, t)",
                "bodies": [dict(tile, count=500, layout="pile")],
                "impulses": [{"t": 0.3, "at": [0, 0.1, 0], "strength": 8, "radius": 3}]}})]})
# 5 Reveal @bar3: the inbox, sorted, on one pane; the stray red tile lands in place.
scenes.append({"id": "reveal", "duration": 4 * B, "transition": "cut",
               "camera": {"shake": {"amount": 9, "at": 0, "duration": 0.2}}, "layers": [s3d({
    "env": env, "camera": cam([(0, [-1.6, 1.1, 3.6], [0, 0.95, 0], 40, 2.0), (4 * B, [0.9, 1.0, 3.0], [0, 0.95, 0], 46, 2.0)]),
    "objects": [{"type": "pane", "size": [2.4, 1.52, 0.03], "pos": [0, 0.95, 0], "texture": "#inbox", "material": {"base": "#FFFFFF", "glow": 0.6, "roughness": 0.2}},
                dict(tile, count=40, pos="=[ -2.2 + 4.4*rand(i), 0.02, -1.5 + 2.5*rand(i+40)]", rot="=[90, 360*rand(i+9), 0]"),
                dict(red_tile, pos="=[1.55, 1.75 - 0.75*easeOut(min(t/0.5, 1)), 0.45]", rot="=[0, -20, 0]")]}),
    ]})
# 6 Feature 1: sorting = tiles falling into order (two angles, cut on beat 15)
dom = dict(tile, type="pane", size=[0.4, 0.62, 0.03], count=120, layout="dominoes", path="line")
scenes.append({"id": "f1", "duration": 2 * B, "transition": "cut", "layers": [s3d({
    "env": env, "camera": cam([(0, [2.5, 1.4, 4.5], [0, 0.3, -2], 34, 2.8), (2 * B, [2.0, 1.2, 3.8], [0, 0.3, -2.5], 36, 2.8)]),
    "physics": {"bodies": [dom]}, "objects": [w3("Sorted", (0.05, 0, 1.0), 0.36, WHITE, (0, 30, 0))]})]})
scenes.append({"id": "f1b", "duration": 2 * B, "transition": "cut", "layers": [s3d({
    "env": env, "camera": cam([(0, [-0.5, 0.35, 1.6], [-0.35, 0.3, -3], 50, 2.0), (2 * B, [-0.4, 0.3, 1.2], [-0.35, 0.3, -3], 55, 2.0)]),
    "physics": {"timeScale": 0.4, "start": 0.6, "bodies": [dom]}, "objects": [w3("overnight.", (-0.6, 0, -1.6), 0.2, RED, (0, 35, 0))]})]})
# 7 Feature 2: one-line summaries on the pane, two angles with rack focus
pane = {"type": "pane", "size": [2.4, 1.52, 0.03], "pos": [0, 0.95, 0], "texture": "#inbox", "material": {"base": "#FFFFFF", "glow": 0.6, "roughness": 0.2}}
scenes.append({"id": "f2", "duration": 2 * B, "transition": "cut",
               "camera": {"animate": [{"from": {"x": -1500}, "to": {"x": 0}, "duration": 0.22, "ease": "outExpo"}]}, "layers": [s3d({
    "env": env, "camera": {"keys": [{"t": 0, "pos": [-1.6, 1.3, 2.4], "target": [-1.95, 0.9, 0], "focal": 45, "fstop": 2.0},
                                    {"t": 2 * B, "pos": [-1.4, 1.25, 2.2], "target": [-1.85, 0.88, 0], "focal": 48, "fstop": 2.0}]},
    "objects": [pane, w3("Every thread,", (-1.82, 0.8, 0.6), 0.15, WHITE, (0, 18, 0))]})]})
scenes.append({"id": "f2b", "duration": 2 * B, "transition": "cut", "layers": [s3d({
    "env": env, "camera": {"keys": [{"t": 0, "pos": [0.9, 1.2, 1.5], "target": [-1.25, 1.0, 0], "focal": 60, "fstop": 1.8},
                                    {"t": 2 * B, "pos": [0.7, 1.1, 1.35], "target": [-1.2, 0.95, 0], "focal": 62, "fstop": 1.8}]},
    "objects": [pane, w3("one line.", (-0.8, 0.82, 0.45), 0.13, RED, (0, 25, 0))]})]})
# 8 Feature 3: focus — everything else dims; then a snap in on the one that matters
pane_f = dict(pane, texture="#inbox_focus")
scenes.append({"id": "f3", "duration": 2 * B, "transition": "cut", "layers": [s3d({
    "env": env, "camera": cam([(0, [0.6, 0.7, 3.4], [0, 0.6, 0], 38, 2.8), (2 * B, [0.4, 0.68, 3.1], [0, 0.6, 0], 40, 2.8)]),
    "objects": [pane_f, w3("Only what", (0, 0, 0.9), 0.3, WHITE)]})]})
scenes.append({"id": "f3b", "duration": 2 * B, "transition": "cut", "camera": {"shake": {"amount": 6, "at": 0, "duration": 0.12}}, "layers": [s3d({
    "env": {"preset": "cyc", "color": PAPER}, "camera": cam([(0, [-0.4, 0.45, 2.6], [0, 0.4, 0.6], 40, 2.4), (2 * B, [-0.25, 0.42, 2.35], [0, 0.4, 0.6], 42, 2.4)]),
    "objects": [pane_f, w3("needs you.", (0, 0, 0.9), 0.24, RED)]})]})
# 9 Proof: a demonstration — 3D numerals count the unread to zero on a red cyc
scenes.append({"id": "proof", "duration": 2 * B, "transition": "cut", "camera": {"shake": {"amount": 6, "at": 0, "duration": 0.12}}, "layers": [s3d({
    "env": {"preset": "cyc", "color": SIG}, "camera": cam([(0, [0, 0.55, 3.0], [0, 0.45, 0], 40, 4), (2 * B, [0, 0.5, 2.7], [0, 0.45, 0], 42, 4)]),
    "objects": [{"text": str(v), "height": 0.9, "depth": 0.3, "bevel": 0.02, "font": FD, "weight": 900, "material": {"base": INK, "roughness": 0.25, "clearcoat": 1},
                 "start": k * 0.1, **({"end": (k + 1) * 0.1} if v else {})} for k, v in enumerate([214, 171, 126, 88, 53, 24, 9, 0])]})]})
# 10 Logo + CTA: the tiles settle into one neat stack beside the name
end3d = lambda keys: s3d({"env": env, "camera": cam(keys),
    "objects": [dict(tile, count=24, pos="=[1.25, 0.011 + 0.021*i + max(0, 2.5 - 3*(t + %s) + 0.12*i), 0]", rot="=[90, 3*sin(i*7.1), 0]"),
                {"text": "Sift", "height": 0.7, "material": "chrome", "pos": [-0.45, 0, 0]}]})
e1 = end3d([(0, [-0.6, 0.5, 2.0], [-0.4, 0.35, 0], 55, 2.0), (2 * B, [-0.3, 0.45, 1.8], [-0.3, 0.35, 0], 58, 2.0)])
e1["props"]["objects"][0]["pos"] %= "0"
e2 = end3d([(0, [-0.45, 0.75, 3.1], [-0.2, 0.38, 0], 40, 4), (4 * B, [-0.45, 0.72, 2.85], [-0.2, 0.38, 0], 41, 4)])
e2["props"]["objects"][0]["pos"] %= str(2 * B)
scenes.append({"id": "logo", "duration": 2 * B, "transition": "cut", "layers": [e1]})
scenes.append({"id": "end", "duration": 4 * B, "transition": "cut", "layers": [e2,
    {"text": "sift.mail", "font": FM, "size": "3.6vh", "color": "#FF8A70", "x": "50%", "y": "87%", "enter": {"preset": "fadeUp", "delay": 0.45}}]})

spec = {"size": "16:9", "fps": 30, "intent": {"pace": "fast", "genre": "launch"}, "motionBlur": False,
        "music": {"style": "house", "bpm": 128, "key": "Fm", "mood": "driving", "hits": ["@bar3", "@bar7"], "end": "ring"},
        "audio": [{"sfx": "whoosh", "at": 2 * B}, {"sfx": "riser", "at": 8 * B}, {"sfx": "impact", "at": 8 * B},
                  {"sfx": "whoosh", "at": 16 * B}, {"sfx": "click", "at": 20 * B}, {"sfx": "click", "at": 22 * B},
                  {"sfx": "impact", "at": 24 * B}, {"sfx": "chime", "at": 26.5 * B}],
        "post": {"preset": "punchy", "grain": 0.04},
        "compositions": {"inbox": {"size": [1200, 760], "duration": 15, "background": PAPER, "layers": inbox_layers()},
                         "inbox_focus": {"size": [1200, 760], "duration": 15, "background": PAPER, "layers": inbox_layers(focus=0)}},
        "scenes": scenes}
json.dump(spec, open(__file__.replace("make.py", "sift.json"), "w"), indent=1)
