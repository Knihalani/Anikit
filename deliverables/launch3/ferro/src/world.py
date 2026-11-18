"""Shared geometry for the Ferro toss film: pan, food, letters."""
import math

IRON = {"texture": "ambientcg:Metal063", "scale": 0.35, "color": "#3A3633", "roughness": 0.42,
        "metallic": 0.45, "imperfections": 0.35}
IRON_IN = dict(IRON, color="#2C2927", roughness=0.32)

def pan_parts(name="pan"):
    prof = [[0, 0], [0.112, 0], [0.122, 0.004], [0.128, 0.014], [0.134, 0.044], [0.131, 0.049],
            [0.126, 0.048], [0.121, 0.017], [0.114, 0.009], [0.1, 0.007], [0, 0.007]]
    return [
        {"type": "lathe", "name": name, "segments": 160, "smooth": True, "points": prof, "material": IRON},
        {"type": "tube", "points": [[0.128, 0.040, 0], [0.18, 0.050, 0], [0.24, 0.058, 0], [0.30, 0.064, 0]],
         "radius": 0.0115, "material": IRON},
        {"type": "sphere", "radius": 0.0135, "pos": [0.30, 0.064, 0], "scale": [1, 0.9, 1], "material": IRON},
        {"type": "box", "size": [0.05, 0.011, 0.055], "radius": 0.009, "pos": [-0.148, 0.043, 0], "material": IRON},
    ]

def pancake(pos, name="pancake"):
    return {"type": "lathe", "name": name, "segments": 96, "smooth": True, "pos": pos,
            "points": [[0, 0], [0.074, 0], [0.081, 0.003], [0.083, 0.006], [0.079, 0.010], [0, 0.011]],
            "material": {"base": "#C98A3E", "roughness": 0.78, "imperfections": 0.6}}

def egg(pos, name="egg"):
    x, y, z = pos
    return [
        {"type": "lathe", "name": name, "segments": 64, "smooth": True, "pos": [x, y, z], "scale": [1, 1, 0.86],
         "points": [[0, 0], [0.05, 0], [0.056, 0.003], [0.05, 0.006], [0.03, 0.007], [0, 0.008]],
         "material": {"base": "#F2EEE6", "roughness": 0.25}},
        {"type": "sphere", "radius": 0.019, "pos": [x + 0.006, y + 0.008, z - 0.004], "scale": [1, 0.62, 1],
         "material": {"base": "#F09A12", "roughness": 0.12}},
    ]

def tomato(pos):
    return {"type": "sphere", "radius": 0.026, "pos": pos, "scale": [1, 0.62, 1],
            "material": {"base": "#B8301C", "roughness": 0.22}}

def mushroom(pos):
    return {"type": "lathe", "segments": 48, "smooth": True, "pos": pos,
            "points": [[0, 0], [0.008, 0], [0.008, 0.012], [0.024, 0.012], [0.027, 0.018], [0.02, 0.028], [0, 0.031]],
            "material": {"base": "#8C6A4E", "roughness": 0.7}}

def toast(pos):
    return {"type": "box", "size": [0.095, 0.012, 0.09], "radius": 0.008, "pos": pos,
            "material": {"base": "#C2833E", "roughness": 0.85, "imperfections": 0.6}}

def toss(t0, dur, h, n=None, spin=(0, 0, 0)):
    """Dense keys for a ballistic hop: rises h metres and falls back over dur seconds, starting at t0."""
    n = n or max(8, int(dur * 24))
    keys = [{"t": 0, "pos": [0, 0, 0], "rot": [0, 0, 0]}] if t0 > 0 else []
    if t0 > 0:
        keys.append({"t": t0, "pos": [0, 0, 0], "rot": [0, 0, 0]})
    for i in range(1, n + 1):
        u = i / n
        keys.append({"t": round(t0 + u * dur, 4), "pos": [0, round(4 * h * u * (1 - u), 4), 0],
                     "rot": [round(spin[0] * u, 2), round(spin[1] * u, 2), round(spin[2] * u, 2)]})
    for k in keys:
        k["ease"] = "linear"
    return keys
