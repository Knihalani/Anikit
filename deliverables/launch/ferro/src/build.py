"""Builds ../spec.json for the Ferro launch film. Run: python3 src/build.py (from deliverables/launch/ferro)."""
import json, os, copy

WORN = {"texture": "ambientcg:Metal063", "scale": 0.3, "color": "#C9B4A0", "roughness": 0.45, "metallic": 0.15}
BASE = {"texture": "ambientcg:Metal063", "scale": 0.3, "color": "#6A5E54", "roughness": 0.7, "metallic": 0.25}
IRON = {"texture": "ambientcg:Metal063", "scale": 0.3, "color": "#6A5E54", "roughness": 0.62, "metallic": 0.6}
PROFILE = [[0,0],[0.105,0],[0.118,0.004],[0.126,0.016],[0.131,0.046],[0.124,0.047],[0.119,0.018],[0.112,0.010],[0.10,0.008],[0,0.008]]
H = 0.047

def pan(keys=None, flipped=False):
    pts = [[r, round(H - y, 4)] for r, y in PROFILE] if flipped else PROFILE
    hy = 0.007 if flipped else 0.04
    objs = [
        {"type": "lathe", "name": "pan", "smooth": False, "segments": 128, "points": pts, "material": IRON},
        {"type": "box", "size": [0.15, 0.016, 0.026], "radius": 0.006, "pos": [0.195, hy, 0], "rot": [0, 0, -8 if flipped else 8], "material": IRON},
        # pouring lip / helper handle opposite
        {"type": "box", "size": [0.05, 0.012, 0.05], "radius": 0.008, "pos": [-0.145, hy, 0], "material": IRON},
    ]
    if keys:
        for o in objs: o["keys"] = keys
    return objs

def sun(az, intensity="14", elev=0.8, dist=1.6, angle=10):
    """A shaft of sun: a hard spot circling the pan. az is an expression of t (radians)."""
    return {"pos": f"=[{dist}*cos({az}), {elev}, {dist}*sin({az})]", "target": [0, 0.02, 0],
            "angle": angle, "penumbra": 0.5, "color": "#FFC98A", "intensity": intensity}

def shot(dur, cam, year=None, at=(0.0, 0.22), size=0.08, az="0.32", intensity="14", objects=None,
         slide=None, extra=None, transition=None, angle=10, gobo_start=0.0, label=None, elev=0.8):
    s3 = {"env": {"preset": "studio"}, "rig": {"preset": "window", "angle": 75, "height": 16, "warmth": 0.7, "ratio": 16, "key": 1.6},
          "floor": {"texture": "ambientcg:Travertine009", "scale": 0.6},
          "camera": cam, "lights": [sun(az, intensity, elev=elev, angle=angle)], "objects": objects or pan()}
    if year:
        g = {"text": year, "font": "Besley", "weight": 800, "size": size, "at": list(at), "invert": True, "strength": 0.97}
        if slide: g.update({"from": slide[0], "settle": slide[1], "start": gobo_start})
        s3["gobo"] = g
    if extra: s3.update(extra)
    sc = {"duration": dur, "scene3d": s3, "layers": []}
    if transition: sc["transition"] = transition
    return sc

def cam(keys, lens=None, shake="breath"):
    c = {"keys": keys, "shake": shake}
    if lens: c["lens"] = lens
    return c

def k(t, pos, target, fstop=5.6, ease=None, focal=None):
    d = {"t": t, "pos": pos, "target": target, "fstop": fstop}
    if ease: d["ease"] = ease
    if focal: d["focal"] = focal
    return d


def mix(c0, c1, f):
    a = [int(c0[i:i+2], 16) for i in (1, 3, 5)]; b = [int(c1[i:i+2], 16) for i in (1, 3, 5)]
    return "#" + "".join(f"{round(x + (y - x) * f):02X}" for x, y in zip(a, b))

def seasoned(f):
    """The pan's seasoning deepens over the century: f 0 (2026) -> 1 (2126)."""
    return {"texture": "ambientcg:Metal063", "scale": 0.3, "color": mix("#8A7A6C", "#2A2522", f),
            "roughness": round(0.78 - 0.4 * f, 3), "metallic": round(0.35 + 0.3 * f, 3)}

def pan_at(f, keys=None):
    objs = pan(keys)
    for o in objs: o["material"] = seasoned(f)
    return objs

STONEWARE = {"extends": "ceramic", "base": "#CDBFA6", "roughness": 0.45, "imperfections": 0.3}
GLAZE = {"extends": "ceramic", "base": "#5B4A3C", "roughness": 0.3, "imperfections": 0.3}
WOOD = {"base": "#8A6440", "roughness": 0.7, "imperfections": 0.4}
def egg(x, z): return {"type": "sphere", "radius": 0.022, "scale": [1, 1.3, 1], "pos": [x, 0.0286, z], "material": {"base": "#E6D3B8", "roughness": 0.6}}
def mug(x, z, m=STONEWARE): return {"type": "lathe", "smooth": False, "pos": [x, 0, z], "material": m,
    "points": [[0, 0], [0.04, 0], [0.042, 0.09], [0.037, 0.09], [0.035, 0.006], [0, 0.006]]}
def bowl(x, z, m=GLAZE): return {"type": "lathe", "smooth": False, "pos": [x, 0, z], "material": m,
    "points": [[0, 0], [0.035, 0], [0.078, 0.052], [0.073, 0.053], [0.032, 0.007], [0, 0.007]]}
def spoon(x, z, yaw): return {"type": "box", "size": [0.2, 0.008, 0.022], "radius": 0.004, "pos": [x, 0.004, z], "rot": [0, yaw, 0], "material": WOOD}

def season(y): return (int(y) - 2026) / 100

scenes = []
# 1 · bar 1 · 2026 — hook: the pan is set down on the stone, low and close; matte, new.
scenes.append(shot(2.5, cam([k(0, [0.42, 0.16, 0.5], [0.0, 0.03, 0.06], 4, focal=50),
                             k(2.5, [0.36, 0.2, 0.44], [0.0, 0.03, 0.08], 4, "out", focal=50)]),
                   year="2026", at=(0.08, 0.24), size=0.06, az="0.25", slide=([-0.25, 0], 1.2), gobo_start=0.4,
                   objects=pan_at(0, keys=[{"t": 0, "pos": [0, 0.07, 0]}, {"t": 0.32, "pos": [0, 0, 0], "ease": "in"}])))
# 2 · bar 2 · 2031 — overhead: breakfast; the sun begins to walk round, handle shadow as clock hand.
scenes.append(shot(2.5, cam([k(0, [0.02, 1.05, 0.06], [0, 0, 0.02], 8, focal=40), k(2.5, [0.0, 0.95, 0.05], [0, 0, 0.02], 8, "inOut", focal=40)]),
                   year="2031", at=(0.0, 0.2), size=0.07, az="0.3+t*0.9", slide=([0.0, 0.25], 0.8),
                   objects=pan_at(season(2031)) + [egg(-0.2, -0.1), egg(-0.16, -0.15), egg(-0.235, -0.16)]))
# 3 · bar 3 · 2058 — the hero 3/4: days flicker; a mug left beside it.
scenes.append(shot(2.5, cam([k(0, [0.08, 0.66, 0.74], [0, 0, 0.1], 8, focal=50), k(2.5, [-0.04, 0.6, 0.7], [0, 0, 0.1], 8, "inOut", focal=50)]),
                   year="2058", at=(0.0, 0.22), az="1.4+t*1.8", intensity="=14*(0.7+0.3*sin(t*14))",
                   objects=pan_at(season(2058)) + [mug(-0.21, -0.1)]))
# 4 · bar 4 · 2084 — handle close: a bowl and a wooden spoon; light racing.
scenes.append(shot(2.5, cam([k(0, [0.5, 0.32, 0.36], [0.17, 0.0, 0.1], 5.6, focal=70), k(2.5, [0.46, 0.3, 0.42], [0.17, 0.0, 0.11], 5.6, "inOut", focal=70)]),
                   year="2084", at=(0.2, 0.17), size=0.05, az="t*4", intensity="=15*(0.75+0.25*sin(t*26))",
                   objects=pan_at(season(2084)) + [bowl(0.06, -0.24), spoon(0.3, -0.06, 30)]))
# 5 · bars 5–6a · years race: six cuts, six cameras, the table's life changing each time.
race = [
    ("2099", 0.9, [0.0, 1.0, 0.12], [0, 0, 0.06], 35, [mug(0.2, -0.16, GLAZE), egg(-0.22, -0.12)]),
    ("2107", 0.7, [0.6, 0.26, 0.62], [0, 0, 0.15], 40, [bowl(-0.24, -0.08, STONEWARE)]),
    ("2113", 0.6, [-0.5, 0.6, 0.55], [0, 0, 0.1], 40, [spoon(-0.24, -0.1, -60), mug(0.18, -0.2)]),
    ("2119", 0.55, [0.06, 0.26, 0.55], [0, 0, 0.22], 70, []),
    ("2123", 0.5, [0.0, 0.2, 0.72], [0, 0.03, 0.05], 50, [egg(0.2, -0.12), egg(0.24, -0.08)]),
    ("2125", 0.5, [0.3, 1.1, 0.9], [0, 0, 0.08], 35, [bowl(0.22, -0.18), mug(-0.24, -0.1, GLAZE)]),
]
for i, (y, d, pos, tgt, focal, props) in enumerate(race):
    end = [pos[0] * 0.95, pos[1] * 0.96, pos[2] * 0.95]
    scenes.append(shot(d, cam([k(0, pos, tgt, 8, focal=focal), k(d, end, tgt, 8, focal=focal)], shake="handheld"),
                       year=y, at=(0.0, 0.25), size=0.075, az=f"{i*1.1}+t*7", intensity="=15*(0.7+0.3*sin(t*40))", angle=12,
                       objects=pan_at(season(y)) + props))
# 6 · bar 6b · hard stop: 2126 — the light stops dead, held in silence; the pan black and glossy.
scenes.append(shot(1.25, cam([k(0, [0.0, 0.72, 0.8], [0, 0, 0.12], 8, focal=50), k(1.25, [0.0, 0.7, 0.78], [0, 0, 0.12], 8, focal=50)]),
                   year="2126", at=(0.0, 0.24), az="0.32", objects=pan_at(1.0)))

# 7–8 · bars 7–8 · payoff: the pan comes down base-up; its casting reads FERRO · SHEFFIELD · 2026.
# One continuous pull-back; the call to action slides onto the stone in the same sun.
lift = [{"t": 0, "pos": [0, 0.09, 0], "rot": [0, 0, 0]}, {"t": 0.28, "pos": [0, 0, 0], "ease": "in"}]
def mark(keys):
    t = [
        {"type": "text", "text": "FERRO", "font": "Besley", "weight": 800, "height": 0.032, "depth": 0.0025, "bevel": 0.0008,
         "pos": [0, H + 0.0024, 0.014], "rot": [-90, 0, 0], "material": WORN},
        {"type": "text", "text": "SHEFFIELD", "font": "Besley", "weight": 700, "height": 0.011, "depth": 0.0018, "tracking": 0.12,
         "pos": [0, H + 0.0017, 0.045], "rot": [-90, 0, 0], "material": WORN},
        {"type": "text", "text": "2026", "font": "Besley", "weight": 700, "height": 0.011, "depth": 0.0018, "tracking": 0.1,
         "pos": [0, H + 0.0017, -0.038], "rot": [-90, 0, 0], "material": WORN},
        {"type": "torus", "radius": 0.088, "tube": 0.0025, "pos": [0, H, 0], "rot": [90, 0, 0], "material": BASE},
    ]
    for o in t: o["keys"] = keys
    return t
payoff_objs = pan(keys=lift, flipped=True) + mark(lift)
for o in payoff_objs:
    if o.get("material") is IRON: o["material"] = BASE
scenes.append(shot(5.0, cam([k(0, [0.04, 0.46, 0.4], [0, 0.04, 0.0], 8, focal=50),
                             k(1.6, [0.035, 0.44, 0.38], [0, 0.04, 0.0], 8, focal=50),
                             k(5.0, [0.06, 0.64, 0.78], [0, 0.02, 0.15], 8, "inOut", focal=50)]),
                   year="Back it on Kickstarter", at=(0.0, 0.3), angle=7, size=0.026, slide=([0.0, 0.18], 1.0), gobo_start=2.5,
                   az="2.3+t*0.05", elev=0.45, intensity="16", objects=payoff_objs))

spec = {
    "size": [1080, 1080], "fps": 24,
    "music": {"bpm": 96, "key": "Dm", "end": "ring", "tracks": [
        {"inst": "cello", "notes": "o2 D2/1 | D2/1 | Bb1/1 | Bb1/1 | G1/1 | A1/2 A1/4 r4 | D2/1 | D2/1", "gain": -2, "reverb": 0.35},
        {"inst": "strings", "notes": "o4 [D3 F3 A3]/1 | [D3 F3 A3]/1 | [D3 F3 Bb3]/1 | [D3 G3 Bb3]/1 | [D3 G3 Bb3]/1 | [E3 A3 C#4]/2 r2 | [D3 F#3 A3 D4]/1 | [D3 F#3 A3 D4]/1", "gain": -8, "reverb": 0.5},
        {"inst": "piano", "notes": "o5 r2 A4/4 D5/4 | F5/2 E5/4 D5/4 | r2 D5/4 F5/4 | A5/2 G5/4 F5/4 | E5/8 F5/8 G5/8 A5/8 Bb5/8 A5/8 G5/8 F5/8 | E5/8 F5/8 G5/8 A5/8 r2 | F#5/2. A5/4 | D6/1", "gain": -6, "reverb": 0.45},
        {"inst": "drums", "drums": {"rim": "x...x...x...x...|x...x...x...x...|x.x.x.x.x.x.x.x.|x.x.x.x.x.x.x.x.|xxxxxxxxxxxxxxxx|xxxxxxxx........|................|................"}, "gain": -10},
    ]},
    "sound": {"palette": "organic", "space": "room", "density": 0.4, "cues": [
        {"sfx": "metal", "at": 0.32, "pitch": -12, "gain": -4},
        {"sfx": "impact+metal", "at": 15.28, "pitch": -10, "gain": -2},
    ]},
    "post": {"grain": 0.05}, "transition": "cut",
    "scenes": scenes,
}
here = os.path.dirname(os.path.abspath(__file__))
json.dump(spec, open(os.path.join(here, "..", "spec.json"), "w"), indent=1)
print("wrote spec.json:", sum(s["duration"] for s in scenes), "s,", len(scenes), "scenes")
