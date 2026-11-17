#!/usr/bin/env python3
"""Proof specs for the CC0 asset library: one scene under 4 HDRIs, 6 PBR
materials, 3 glTF models. Writes JSON next to this file; render with

    for f in examples/video/asset-library/*.json; do
      anikit-video still "$f" -o docs/video/shots/asset-library/$(basename "$f" .json).jpg
    done
"""
import json, os

HERE = os.path.dirname(os.path.abspath(__file__))
LABEL = {"font": "JetBrains Mono", "size": "1.5vh", "letterSpacing": "0.12em", "uppercase": True}


def label(text, x, y, color="#FFFFFFB3", anchor="left"):
    return {"text": text, **LABEL, "color": color, "x": x, "y": y, "anchor": anchor}


def spec(props, overlays, bg="#000000"):
    return {"size": [1920, 1080], "fps": 30, "background": bg, "scenes": [{"duration": 1, "layers": [{"component": "scene3d", "props": props}] + overlays}]}


def write(name, s):
    with open(os.path.join(HERE, name + ".json"), "w") as f:
        json.dump(s, f, indent=1)


# 1. One scene, four HDRIs: light, reflections, background and floor all come from the photo.
HERO_OBJS = [
    {"type": "cylinder", "radius": 0.42, "height": 0.5, "pos": [0, 0.25, 0], "material": {"texture": "ambientcg:Terrazzo018", "scale": 1.5}},
    {"type": "model", "src": "polyhaven:model/marble_bust_01", "height": 0.85, "pos": [0, 0.5, 0], "rot": [0, -20, 0]},
    {"type": "sphere", "radius": 0.3, "pos": [-1.0, 0.3, 0.35], "material": "chrome"},
    {"type": "sphere", "radius": 0.3, "pos": [1.0, 0.3, 0.3], "material": {"base": "#E8E4DC", "roughness": 0.6}},
]
for name, rot, exp in [("studio_small_09", 40, 0), ("kloppenheim_06", 200, 0), ("je_gray_park", 150, 0.8), ("hansaplatz", 60, -0.3)]:
    props = {
        "env": {"hdri": f"polyhaven:hdri/{name}", "rotate": rot, "exposure": exp},
        "camera": {"keys": [{"t": 0, "pos": [0.45, 0.8, 5.3], "target": [0, 0.55, 0], "focal": 50, "fstop": 2.8}]},
        "objects": HERO_OBJS,
    }
    write(f"hdri-{name}", spec(props, [label(f"env.hdri  polyhaven:hdri/{name}", "4%", "93%")]))

# 2. Six PBR sets on the same sphere, triplanar (no UVs), under one studio HDRI.
SPACING = 15.6  # % of the frame width between sphere centres (matches the camera below)
MATS = ["ambientcg:Wood049", "ambientcg:Marble012", "polyhaven:texture/rusty_metal_02", "polyhaven:texture/fabric_pattern_07", "ambientcg:Terrazzo013", "ambientcg:Marble016"]
objs, overlays = [], []
for i, m in enumerate(MATS):
    x = (i - 2.5) * 1.12
    objs.append({"type": "sphere", "radius": 0.46, "pos": [x, 0.46, 0], "rot": [0, 25 * i, 0], "material": {"texture": m}})
    overlays.append(label(m.split(":")[1].split("/")[-1], f"{50 + (i - 2.5) * SPACING:.2f}%", "57.5%", "#1A1A1AB3", "center"))  # under each sphere
props = {
    "env": {"hdri": "polyhaven:hdri/studio_small_09", "rotate": 30, "background": "blur"},
    "camera": {"keys": [{"t": 0, "pos": [0, 1.4, 11.5], "target": [0, 0.12, 0], "focal": 58, "fstop": 5.6}]},
    "objects": objs,
}
write("materials", spec(props, overlays + [label("material.texture", "4%", "93%", "#1A1A1AB3")]))

# 3. Three textured glTF models on a textured tabletop.
props = {
    "env": {"hdri": "polyhaven:hdri/brown_photostudio_02", "rotate": 230, "background": "blur"},
    "camera": {"keys": [{"t": 0, "pos": [0.2, 0.55, 1.55], "target": [0, 0.16, 0], "focal": 50, "fstop": 2.8}]},
    "floor": False,
    "objects": [
        {"type": "box", "size": [3.2, 0.06, 2.0], "pos": [0, -0.03, -0.3], "material": {"texture": "polyhaven:texture/wood_table_001", "scale": 1}},
        {"type": "model", "src": "polyhaven:model/Camera_01", "height": 0.16, "pos": [-0.32, 0, 0.08], "rot": [0, 35, 0]},
        {"type": "model", "src": "polyhaven:model/potted_plant_02", "height": 0.42, "pos": [0.05, 0, -0.32], "rot": [0, 0, 0]},
        {"type": "model", "src": "polyhaven:model/food_apple_01", "height": 0.075, "pos": [0.3, 0, 0.12], "rot": [0, 70, 0]},
    ],
}
write("models", spec(props, [label("polyhaven:model/Camera_01 · potted_plant_02 · food_apple_01", "4%", "93%")]))
print("ok")
