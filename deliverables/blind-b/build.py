#!/usr/bin/env python3
"""Builds spec.json for "Halden" (15 s, 128 BPM). Every time is a beat: b(n) = n * 0.46875 s.

Direction
  Logline: for people who buy one knife for life - a Halden blade is drawn at 1,450 C,
           struck 4,000 times, folded into 256 layers and quenched in one breath.
  Feeling: awe at heat and weight.
  World:   a black forge lit only by the steel itself. Bone serif type, molten accent.
  Signature: raymarched anvil + white-hot billet whose glow lights the scene (line light,
           glossy reflections), cut between five angles on the hammer strikes.
"""
import json, math

BEAT = 60 / 128
def b(n): return round(n * BEAT, 4)

BONE = "#EDE6D8"; MOLTEN = "#FF7A1F"; HOT = "#FFE2A8"
SERIF = "Instrument Serif"; MONO = "JetBrains Mono"

def project(camA, camH, camD, lens, camX=0.0, p=(0.25, 0.17, 0.0)):
    tgt = (camX, 0.05, 0.0)
    ro = (tgt[0] + math.sin(camA) * camD, tgt[1] + camH * camD * 0.6, tgt[2] + math.cos(camA) * camD)
    f = [tgt[i] - ro[i] for i in range(3)]; n = math.sqrt(sum(x * x for x in f)); f = [x / n for x in f]
    r = [f[1] * 0 - f[2] * 1, f[2] * 0 - f[0] * 0, f[0] * 1 - f[1] * 0]  # cross(f, up)
    n = math.sqrt(sum(x * x for x in r)); r = [x / n for x in r]
    u = [r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0]]
    e = [p[i] - ro[i] for i in range(3)]
    z = sum(e[i] * f[i] for i in range(3))
    sx = sum(e[i] * r[i] for i in range(3)) * lens / z
    sy = sum(e[i] * u[i] for i in range(3)) * lens / z
    return [round((960 + sx * 1080) / 1920, 4), round((540 - sy * 1080) / 1080, 4)]

def label(text, x, y, at=0.0, size=26, anchor="topLeft", color="#EDE6D8B0"):
    return {"text": text, "font": MONO, "size": size, "letterSpacing": 0.18, "uppercase": True, "color": color,
            "x": x, "y": y, "anchor": anchor, "enter": {"preset": "typewriter", "delay": at, "duration": 0.35}}

def anvil_shot(dur, cam, strike=None, heat=0.95, text=None, extra=None, drift=None):
    """One camera angle on the anvil. `strike` = local time the hammer lands."""
    u = {"heat": heat, "lift": 1.0 if strike is not None else cam.get("lift", 1.0), "flash": 0.0,
         "camA": cam["A"], "camH": cam["H"], "camD": cam["D"], "lens": cam.get("lens", 1.6), "camX": cam.get("X", 0.0),
         "hammer": cam.get("hammer", 1.0)}
    anim = []
    if drift:
        anim.append({"to": drift, "duration": dur, "ease": "linear"})
    layers = [{"src": "shaders/anvil.glsl", "uniforms": u, "animate": anim}]
    scene = {"duration": dur, "transition": "cut", "layers": layers}
    if strike is not None:
        anim += [
            {"to": {"lift": 0.0}, "at": max(strike - 0.2, 0), "duration": min(0.2, strike), "ease": "inCubic"},
            {"to": {"flash": 1.0}, "at": strike, "duration": 0.02, "ease": "linear"},
            {"to": {"flash": 0.0}, "at": strike + 0.02, "duration": 0.4, "ease": "outCubic"},
            {"to": {"lift": 0.55}, "at": strike + 0.06, "duration": 0.5, "ease": "outCubic"},
        ]
        o = project(cam["A"], cam["H"], cam["D"], cam.get("lens", 1.6), cam.get("X", 0.0))
        layers.append({"component": "sparks", "props": {
            "bursts": [strike], "origin": o, "count": 700, "power": cam.get("power", 1.0), "gravity": 1500,
            "life": 1.1, "direction": -80, "spread": 150, "colors": ["#FFF6DC", "#FF5A12"], "seed": int(strike * 100) + 3}})
        layers.append({"component": "sparks", "props": {
            "bursts": [strike + 0.03], "origin": o, "count": 250, "power": cam.get("power", 1.0) * 0.5, "gravity": 900,
            "life": 1.3, "direction": -90, "spread": 300, "colors": ["#FFD27A", "#C2380A"], "seed": int(strike * 100) + 9}})
        scene["camera"] = {"shake": {"amount": 16, "frequency": 18, "at": strike, "duration": 0.32},
                           "animate": [{"keyframes": {"zoom": [1.0, 1.07, 1.03]}, "at": strike, "duration": 0.35, "ease": "outExpo"}]}
    if text: layers += text
    if extra: layers += extra
    return scene

scenes = []

# 1. b0-4  Fire. The temperature counts up; snap-zoom at b2.
scenes.append({"duration": b(4), "layers": [
    {"src": "shaders/forge.glsl", "uniforms": {"heat": 0.0, "zoom": 1.0, "flare": 0.0, "rise": 1.0},
     "animate": [{"to": {"heat": 1.05}, "duration": 0.7, "ease": "outCubic"},
                 {"to": {"flare": 1.0}, "at": b(2), "duration": 0.05}, {"to": {"flare": 0.0}, "at": b(2) + 0.05, "duration": 0.5},
                 {"to": {"zoom": 1.9}, "at": b(2), "duration": 0.22, "ease": "outExpo"},
                 {"to": {"zoom": 2.2}, "at": b(2) + 0.22, "duration": b(2) - 0.22, "ease": "linear"}]},
    {"text": "{value:,}°", "value": 1450, "font": SERIF, "size": 330, "color": HOT, "x": 120, "y": 980, "anchor": "bottomLeft",
     "glow": {"color": MOLTEN, "radius": 40, "intensity": 0.8}, "letterSpacing": -0.02,
     "animate": [{"from": {"value": 0, "opacity": 0, "blur": 20}, "duration": b(1.5), "ease": "outQuart"},
                 {"to": {"scale": 1.12}, "at": b(2), "duration": 0.22, "ease": "outExpo"}]},
    label("Billet temperature · celsius", 128, 600, 0.25),
    label("Halden forge · Sheffield", 1792, 80, 0.1, anchor="topRight"),
]})

# 2-6. The anvil, five angles, four strikes.
scenes.append(anvil_shot(b(2), {"A": 0.45, "H": 0.62, "D": 3.5}, heat=0.95, drift={"camD": 3.0, "camA": 0.6},
    text=[{"text": "Drawn from the fire.", "font": SERIF, "size": 92, "color": BONE, "x": 120, "y": 960, "anchor": "bottomLeft",
           "enter": "mask by line up"},
          label("No. 0417 · 1084 / 15N20", 120, 80, 0.15)]))
scenes.append(anvil_shot(b(2), {"A": 1.25, "H": 0.18, "D": 2.1, "power": 1.2}, strike=b(1), drift={"camA": 1.18}))
scenes.append(anvil_shot(b(2), {"A": -0.45, "H": 1.25, "D": 2.5}, strike=b(1), drift={"camA": -0.38},
    text=[{"text": "Struck", "font": SERIF, "size": 120, "color": BONE, "x": 1800, "y": 330, "anchor": "topRight",
           "enter": {"preset": "mask by line up", "delay": 0.0}},
          {"text": "{value:,} times.", "value": 4000, "font": SERIF, "size": 120, "color": HOT, "x": 1800, "y": 470, "anchor": "topRight",
           "animate": [{"from": {"value": 1, "opacity": 0}, "at": b(1), "duration": b(1), "ease": "outExpo"}]}]))
scenes.append(anvil_shot(b(1), {"A": 0.95, "H": 0.32, "D": 1.35, "lens": 1.4, "power": 1.4}, strike=b(0.5), drift={"camD": 1.2}))
scenes.append(anvil_shot(b(1) + 0.3, {"A": 0.02, "H": 2.4, "D": 1.7, "power": 1.1}, strike=b(0.5), drift={"camA": 0.3}))

# 7. b12-16  Folding: the strata double every half beat.
fold = [{"to": {"layers": 2.0 ** (k + 2)}, "at": b(k * 0.5), "duration": 0.14, "ease": "outExpo"} for k in range(7)]
scenes.append({"duration": b(4), "transition": {"type": "zoom-blur", "duration": 0.3}, "layers": [
    {"src": "shaders/damascus.glsl", "uniforms": {"layers": 3.0, "light": -0.6, "zoom": 1.0, "glow": 0.55},
     "animate": fold + [{"to": {"light": 0.9, "zoom": 1.8, "glow": 0.0}, "duration": b(4), "ease": "inOutSine"}]},
    {"text": "×{value}", "value": 256, "font": SERIF, "size": 300, "color": BONE, "x": 1820, "y": 1000, "anchor": "bottomRight",
     "shadow": {"x": 0, "y": 10, "blur": 40, "color": "#000000C0"},
     "animate": [{"keyframes": {"value": [[0, 2], [0.125, 4], [0.25, 8], [0.375, 16], [0.5, 32], [0.625, 64], [0.75, 128], [0.875, 256], [1, 256]]},
                  "duration": b(4), "ease": "steps(1)"}]},
    {"text": "Folded", "font": SERIF, "size": 92, "color": BONE, "x": 1820, "y": 640, "anchor": "bottomRight", "enter": "mask by line up",
     "shadow": {"x": 0, "y": 6, "blur": 30, "color": "#000000C0"}},
    label("Layers of steel", 1806, 1012, 0.3, anchor="topRight"),
]})

# 8. b16-18  The blade hangs over the oil.
Q = {"dip": 0.0, "cool": 0.0, "steam": 0.0, "heat": 1.0, "camA": 0.5, "camH": 0.32, "camD": 2.7, "camY": 0.35, "lens": 1.5}
scenes.append({"duration": b(2), "layers": [
    {"src": "shaders/quench.glsl", "uniforms": dict(Q),
     "animate": [{"to": {"camD": 2.2, "camY": 0.25, "dip": 0.08}, "duration": b(2), "ease": "inQuad"}]},
    label("Quench · 52 °C oil", 120, 80, 0.1),
]})
# 9. b18-20  Plunge.
scenes.append({"duration": b(2), "camera": {"shake": {"amount": 18, "frequency": 14, "at": 0.0, "duration": 0.4}},
  "layers": [
    {"src": "shaders/quench.glsl", "uniforms": dict(Q, dip=0.1, camA=0.15, camH=1.05, camD=2.3, camY=0.1),
     "animate": [{"to": {"dip": 0.85}, "duration": 0.18, "ease": "outExpo"},
                 {"to": {"steam": 1.0}, "duration": 0.25, "ease": "outCubic"},
                 {"to": {"cool": 0.45}, "duration": b(2), "ease": "outCubic"},
                 {"to": {"camD": 2.0}, "duration": b(2), "ease": "outCubic"}]},
    {"text": "Quenched in", "font": SERIF, "size": 120, "color": BONE, "x": 120, "y": 760, "anchor": "bottomLeft",
     "enter": {"preset": "mask by line up", "delay": b(0.5)}, "shadow": {"x": 0, "y": 6, "blur": 30, "color": "#000000D0"}},
    {"text": "*one breath.*", "font": SERIF, "size": 120, "color": HOT, "x": 120, "y": 900, "anchor": "bottomLeft",
     "enter": {"preset": "mask by line up", "delay": b(1)}, "shadow": {"x": 0, "y": 6, "blur": 30, "color": "#000000D0"}},
]})
# 10. b20-22  Closer, the cooling front climbs.
scenes.append({"duration": b(2), "layers": [
    {"src": "shaders/quench.glsl", "uniforms": dict(Q, dip=0.85, steam=1.0, cool=0.45, camA=1.05, camH=0.06, camD=1.25, camY=0.32, lens=1.3),
     "animate": [{"to": {"cool": 1.0, "steam": 0.8, "camY": 0.42}, "duration": b(2), "ease": "inOutSine"}]},
]})

# 11. b22-25  Cold steel: a glint runs along the pattern.
scenes.append({"duration": b(3) + 0.12, "transition": "cut",
  "camera": {"animate": [{"from": {"x": -260}, "duration": 0.16, "ease": "outExpo"}]},
  "layers": [
    {"src": "shaders/damascus.glsl", "uniforms": {"layers": 72.0, "light": -1.4, "zoom": 1.25, "glow": 0.0},
     "animate": [{"to": {"light": 1.6}, "duration": b(3), "ease": "inOutExpo"}]},
    {"text": "Made to", "font": SERIF, "size": 170, "color": BONE, "x": 120, "y": 520, "anchor": "bottomLeft",
     "enter": "mask by line up", "shadow": {"x": 0, "y": 8, "blur": 40, "color": "#000000E0"}},
    {"text": "*outlast you.*", "font": SERIF, "size": 170, "color": BONE, "x": 120, "y": 720, "anchor": "bottomLeft",
     "enter": {"preset": "mask by line up", "delay": b(0.75)}, "shadow": {"x": 0, "y": 8, "blur": 40, "color": "#000000E0"}},
]})

# 12. b25-32  The mark.
scenes.append({"duration": b(7), "transition": {"type": "fadeWhite", "duration": 0.12},
  "camera": {"aperture": 6, "focus": 0,
             "shake": {"amount": 12, "frequency": 16, "at": b(3), "duration": 0.3},
             "animate": [{"from": {"zoom": 1.18}, "duration": b(3), "ease": "outExpo"},
                         {"keyframes": {"zoom": [1.0, 0.9, 0.92]}, "at": b(3), "duration": 0.6, "ease": "outExpo"},
                         {"to": {"zoom": 0.88}, "at": b(3) + 0.6, "duration": b(4) - 0.6, "ease": "linear"}]},
  "layers": [
    {"src": "shaders/forge.glsl", "uniforms": {"heat": 0.75, "zoom": 1.4, "flare": 0.0, "rise": 0.7}, "depth": 2.0, "opacity": 0.55},
    {"component": "sparks", "props": {"bursts": [0.0, b(3)], "origin": [0.5, 0.7], "count": 700, "power": 1.3, "gravity": 1100,
                                       "life": 1.4, "direction": -90, "spread": 150, "colors": ["#FFF6DC", "#FF5A12"], "seed": 41}},
    {"component": "text-3d", "props": {"text": "HALDEN", "font": SERIF, "weight": 400, "color": "#D8C9B2", "metallic": 0.95, "roughness": 0.2,
                                        "tracking": 0.06, "depth": 0.16, "intro": "drop", "stagger": 0.05, "sway": 4, "shine": 1.5, "angle": -8, "tilt": 6},
     "width": 1700, "height": 520, "y": 470},
    {"text": "Hand-forged knives · Sheffield, since 1911", "font": SERIF, "size": 54, "color": BONE, "y": 860, "shadow": {"x": 0, "y": 4, "blur": 24, "color": "#000000E0"},
     "enter": {"preset": "mask by line up", "delay": b(3)}},
    {"shape": "line", "points": [[760, 925], [1160, 925]], "stroke": MOLTEN, "strokeWidth": 2,
     "animate": [{"from": {"trimEnd": 0}, "at": b(3.5), "duration": 0.5, "ease": "outExpo"}]},
    label("One knife. For life.", 960, 950, b(4), anchor="top", color="#EDE6D890"),
]})

spec = {
    "size": [1920, 1080], "fps": 30, "background": "#050404",
    "theme": {"colors": {"primary": MOLTEN, "accent": MOLTEN, "bg": "#050404", "text": BONE, "muted": "#9C9283"},
              "fonts": {"heading": SERIF, "body": "Inter", "mono": MONO}},
    "transition": "cut",
    "motionBlur": {"shutter": 180},
    "post": {"preset": "cinematic", "bloom": 0.55, "bloomThreshold": 0.72, "halation": 0.35, "grain": 0.035,
             "vignette": 0.35, "aberration": 0.15, "contrast": 1.08, "saturation": 1.05},
    "audio": [{"src": "score.wav", "volume": 1.0}],
    "scenes": scenes,
}
json.dump(spec, open("spec.json", "w"), indent=1)
print("scenes", len(scenes), "total", round(sum(s["duration"] for s in scenes), 3), "s")
