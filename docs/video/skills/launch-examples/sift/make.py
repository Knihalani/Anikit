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

scenes = []
# 1 Hook: envelopes raining in slow motion, one red one in focus.
scenes.append({"id": "hook", "duration": 2 * B, "layers": [s3d({
    "env": env, "camera": cam([(0, [0, 1.6, 4.2], [0, 1.4, 0], 50, 1.8), (2 * B, [0.2, 1.5, 3.7], [0, 1.3, 0], 52, 1.8)]),
    "objects": [dict(tile, count=180, pos="=[-3 + 6*rand(i), 3.4 - 3.6*fract(rand(i+5) + 0.12*t), -2.5 + 3*rand(i+9)]", rot="=[360*rand(i+1) + 30*t, 360*rand(i+2) + 20*t, 0]")] +
               [dict(red_tile, pos="=[0.1, 1.45 - 0.35*t, 0.6]", rot="=[18, 25 + 40*t, 8]")]})]})
# 2-3 Tension slams
scenes.append(slam("Too much", INK, PAPER, 2 * B, "46%"))
scenes.append(slam("mail.", SIG, INK, 2 * B, "55%", 3))
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
    "physics": {"bodies": [dom]}}),
    {"text": "Sorted", "font": FD, "size": "16vh", "weight": 900, "color": PAPER, "letterSpacing": "-0.04em", "x": "7%", "y": "14%", "anchor": "topLeft",
     "animate": [{"from": {"scale": 1.6, "opacity": 0, "blur": 10}, "duration": 0.15, "ease": "outExpo"}]}]})
scenes.append({"id": "f1b", "duration": 2 * B, "transition": "cut", "layers": [s3d({
    "env": env, "camera": cam([(0, [-0.5, 0.35, 1.6], [0.3, 0.3, -3], 50, 2.0), (2 * B, [-0.4, 0.3, 1.2], [0.3, 0.3, -3], 55, 2.0)]),
    "physics": {"timeScale": 0.4, "start": 0.6, "bodies": [dom]}}),
    {"layers": [{"shape": "rect", "width": 640, "height": 170, "fill": SIG, "x": 0, "y": 0, "anchor": "topLeft"},
                {"text": "*overnight.*", "font": "Instrument Serif", "size": "13vh", "color": INK, "x": 36, "y": 6, "anchor": "topLeft"}],
     "x": "7%", "y": "12%", "anchor": "topLeft", "animate": [{"from": {"scaleX": 0}, "duration": 0.18, "ease": "outExpo"}]}]})
# 7 Feature 2: one-line summaries on the pane, two angles with rack focus
pane = {"type": "pane", "size": [2.4, 1.52, 0.03], "pos": [0, 0.95, 0], "texture": "#inbox", "material": {"base": "#FFFFFF", "glow": 0.6, "roughness": 0.2}}
scenes.append({"id": "f2", "duration": 2 * B, "transition": "cut",
               "camera": {"animate": [{"from": {"x": -1500}, "to": {"x": 0}, "duration": 0.22, "ease": "outExpo"}]}, "layers": [s3d({
    "env": env, "camera": {"keys": [{"t": 0, "pos": [-1.6, 1.3, 2.4], "target": [-1.1, 1.0, 0], "focal": 45, "fstop": 2.0},
                                    {"t": 2 * B, "pos": [-1.4, 1.25, 2.2], "target": [-1.0, 0.98, 0], "focal": 48, "fstop": 2.0}]},
    "objects": [pane]}),
    {"text": "Every thread,", "font": FD, "size": "8vh", "weight": 800, "color": PAPER, "x": "7%", "y": "44%", "anchor": "topLeft",
     "enter": {"preset": "mask by line up", "delay": 0.1}}]})
scenes.append({"id": "f2b", "duration": 2 * B, "transition": "cut", "layers": [s3d({
    "env": env, "camera": {"keys": [{"t": 0, "pos": [0.9, 1.2, 1.5], "target": [-0.9, 1.05, 0], "focal": 60, "fstop": 1.8},
                                    {"t": 2 * B, "pos": [0.7, 1.1, 1.35], "target": [-0.85, 0.98, 0], "focal": 62, "fstop": 1.8}]},
    "objects": [pane]}),
    {"text": "*one line.*", "font": "Instrument Serif", "size": "16vh", "color": SIG, "x": "7%", "y": "40%", "anchor": "topLeft", "enter": "mask by line up"}]})
# 8 Feature 3: focus — everything else dims; then a snap in on the one that matters
scenes.append({"id": "f3", "duration": 2 * B, "transition": "cut", "background": INK,
               "camera": {"animate": [{"from": {"zoom": 1.2}, "to": {"zoom": 1}, "duration": 0.25, "ease": "outExpo"}]}, "layers": [
    {"composition": "inbox_focus", "width": 960, "height": 608, "x": "29%", "y": "50%", "rotation": -2},
    {"text": "Only what", "font": FD, "size": "12vh", "weight": 900, "color": PAPER, "x": "78%", "y": "50%", "letterSpacing": "-0.04em"}]})
scenes.append({"id": "f3b", "duration": 2 * B, "transition": "cut", "background": INK,
               "camera": {"animate": [{"from": {"zoom": 1.25, "rotation": 2}, "to": {"zoom": 1, "rotation": 0}, "duration": 0.2, "ease": "outExpo"}],
                          "shake": {"amount": 6, "at": 0, "duration": 0.12}}, "layers": [
    {"composition": "inbox_focus", "width": 3000, "height": 1900, "x": "62%", "y": "88%", "rotation": -2},
    {"text": "needs you.", "font": FD, "size": "20vh", "weight": 900, "color": SIG, "x": "50%", "y": "22%", "letterSpacing": "-0.05em"}]})
# 9 Proof: a demonstration, the counter runs to zero
scenes.append({"id": "proof", "duration": 2 * B, "transition": "cut", "background": SIG,
               "camera": {"shake": {"amount": 6, "at": 0, "duration": 0.12}}, "layers": [
    *[{"text": str(v), "start": k * 0.1, "end": (k + 1) * 0.1 if v else 99, "font": FD, "size": "40vh", "weight": 900, "color": INK, "letterSpacing": "-0.06em", "x": "50%", "y": "48%", "anchor": "center"}
      for k, v in enumerate([214, 171, 126, 88, 53, 24, 9, 0])],
    {"text": "UNREAD · 06:58", "font": FM, "size": "3.4vh", "color": INK, "x": "50%", "y": "86%"}]})
# 10 Logo + CTA: the tiles settle into one neat stack beside the name
end3d = lambda keys: s3d({"env": env, "camera": cam(keys),
    "objects": [dict(tile, count=24, pos="=[1.9, 0.011 + 0.021*i + max(0, 2.5 - 3*(t + %s) + 0.12*i), 0]", rot="=[90, 3*sin(i*7.1), 0]"),
                {"text": "Sift", "height": 0.7, "material": "chrome", "pos": [-0.45, 0, 0]}]})
e1 = end3d([(0, [-0.6, 0.5, 2.0], [-0.4, 0.35, 0], 55, 2.0), (2 * B, [-0.3, 0.45, 1.8], [-0.3, 0.35, 0], 58, 2.0)])
e1["props"]["objects"][0]["pos"] %= "0"
e2 = end3d([(0, [0, 1.2, 5.2], [0, 0.6, 0], 40, 3.2), (4 * B, [0, 1.15, 4.7], [0, 0.6, 0], 42, 3.2)])
e2["props"]["objects"][0]["pos"] %= str(2 * B)
scenes.append({"id": "logo", "duration": 2 * B, "transition": "cut", "layers": [e1]})
scenes.append({"id": "end", "duration": 4 * B, "transition": "cut", "layers": [e2,
    {"text": "Your inbox, sorted before you wake.", "font": "Inter", "size": "4.4vh", "weight": 500, "color": PAPER, "x": "50%", "y": "80%",
     "enter": {"preset": "fadeUp", "delay": 0.2}},
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
