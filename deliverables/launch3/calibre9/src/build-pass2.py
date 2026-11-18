"""Writes spec.json for the Calibre 9 teaser (one shader world, six shots on a 100 bpm grid)."""
import json, os
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "launch/pass2.json")
FONT = "Noto Serif Display"

def dial(dur, **u):
    base = {"zoom": 1.3, "cx": 0, "cy": 0, "rot": 0, "T": 0, "rate": 0, "expo": 1.0, "ink": 1.0, "tone": 0, "sreal": 0}
    base.update(u)
    return {"id": "dial", "type": "shader", "src": "src/dial-pass2.glsl",
            "channels": [{"layers": [{"composition": "printing"}]}], "uniforms": base}

def shadow_word(text, size, x, y, start=0.0, rot=0, dur=None):
    l = {"text": text, "font": FONT, "weight": 400, "size": size, "x": x, "y": y, "anchor": "left",
         "color": "#140d08", "blend": "multiply", "rotation": rot, "blur": 2,
         "enter": {"preset": "fade", "delay": start, "duration": 0.5}}
    return l

def inlay(name, text, size, sub=None):
    ls = [{"text": text, "font": FONT, "weight": 400, "size": size, "x": 960, "y": 540, "color": "#000"}]
    if sub:
        ls.append({"text": sub, "font": FONT, "weight": 500, "size": round(size*0.27), "letterSpacing": "0.03em",
                   "x": 960, "y": 540 + round(size*0.78), "color": "#000"})
    COMPS[name] = {"layers": ls}
    return {"layers": [{"composition": name}]}

COMPS = {}
NONE = {"layers": []}
def dial2(dur, ch1=NONE, **u):
    l = dial(dur, **u)
    l["channels"] = [{"layers": [{"composition": "printing"}], "mipmaps": True}, ch1]
    return l

scenes = []
# 1a · Friday 17:00, the crown is wound: macro on the knurled crown, lateral drift (bar 1, beats 1–2)
scenes.append({"duration": 1.2, "background": "#000", "layers": [
    dial2(1.2, zoom=0.19, cx="=1.20 + 0.05*t/1.2", cy=0.02, rot=1.25, T=0.0, wind="=t*7", sreal=24.5, ink=0)]})
# 1b · the seconds-hand tip starts stepping; "Friday." inlaid in the bench, in the window light (bar 1, beats 3–4)
scenes.append({"duration": 1.2, "background": "#000", "layers": [
    dial2(1.2, inlay("fri", "Friday.", 230), zoom=0.36, cx="=0.80 + 0.03*t/1.2", cy=-0.86, rot=-0.35, T=0.0, sreal="=25.7 + t",
          ink=0, tcx=1.10, tcy=-1.10, tsc=1700)]})
# 2 · the hand centre, time starts to accelerate, sunset → night; camera rotates (bar 2)
scenes.append({"duration": 2.4, "background": "#000", "layers": [
    dial2(2.4, zoom=0.70, cx=0.10, cy=-0.05, rot="=0.15 + 0.55*t/2.4", ink=0,
          T="=0.0007 + 7.5*pow(t/2.4, 3)", rate="=7.5*3*pow(t/2.4, 2)/2.4", sreal=26.9)]})
# 3 · whole dial, Saturday passes; a whip rotation that settles (bar 3, beats 1–3)
scenes.append({"duration": 1.8, "background": "#000", "layers": [
    dial2(1.8, zoom=1.06, cx=0, cy=0, rot="=-0.9 + 0.75*(1 - pow(1 - t/1.8, 3))", ink=0,
          T="=7.5 + 22.5*t/1.8", rate=22.5/1.8)]})
# 4 · baton at 3, minute hand a blur, Sunday flashes past; lateral slide (bar 3 beat 4 → bar 4 beat 2)
scenes.append({"duration": 1.8, "background": "#000", "layers": [
    dial2(1.8, zoom=0.42, cx="=0.48 + 0.22*t/1.8", cy=0.10, rot=0.8, ink=0,
          T="=30 + 28.4*t/1.8", rate=28.4/1.8)]})
# 5 · hard stop: real time again, the seconds-hand tip alone in the dark (bar 4 beats 3–4)
scenes.append({"duration": 1.2, "background": "#000", "layers": [
    dial2(1.2, zoom=0.16, cx=-0.05, cy=0.80, rot=0.15, T=58.4, sreal="=t", expo=2.6, ink=0)]})
# 6 · Monday dawn rakes in from the right, hands settle at 8:09, the name appears in the dial printing (bar 5)
scenes.append({"duration": 2.4, "background": "#000", "layers": [
    dial2(2.4, inlay("mon", "Monday.", 200, "Calibre 9  ·  150 pieces"),
          zoom="=1.30 + 0.16*(1 - pow(1 - min(t/2.0, 1), 3))", cx=-0.95, cy=0.05, rot=0,
          T="=59.0 + 4.15*(1 - pow(1 - min(t/1.6, 1), 3))",
          rate="=4.15*3*pow(1 - min(t/1.6, 1), 2)/1.6", sreal="=max(t - 1.6, 0)",
          wx=-1.9, wy=0.0, ws=1.6, tcx=-2.33, tcy=0.12, tsc=400, ink=1)]})

# escapement ticks (28,800 vph) in the real-time shots
ticks = [round(1.2 + k/8, 3) for k in range(0, 10)] + [round(8.4 + k/8, 3) for k in range(0, 10)] + [round(11.2 + k/8, 3) for k in range(0, 6)]
cues = [{"sfx": "tick", "at": t, "gain": -9 if t < 8 else -6, "pitch": 7, "x": 0.1} for t in ticks]
cues += [{"sfx": "click", "at": round(0.05 + k*0.11, 3), "gain": -13, "pitch": -5 + (k % 2), "x": 0.3} for k in range(10)]

music = {"bpm": 100, "key": "D", "end": "ring", "followCuts": False, "tracks": [
    {"inst": "piano", "gain": -2, "reverb": 0.35, "notes":
        "r/8 A4/8 D5/8 E5/8 F#5/2 | r/8 A4/8 D5/8 E5/8 A5/2 | (D5/8 A4/8 F#5/8 A4/8)x2 | (D5/16 A4/16 G5/16 B4/16)x2 r/2 | [D4 A4 D5 F#5]/1!"},
    {"inst": "strings", "gain": -8, "reverb": 0.5, "notes":
        "r/1 | [D3 A3 F#4]/1 | [B2 F#3 D4]/1 | [G2 D3 B3]/2 r/2 | [D2 A2 F#3 D4]/1"},
    {"inst": "cello", "gain": -9, "reverb": 0.4, "notes": "r/1 | r/1 | B2/1 | G2/2 r/2 | D2/1"}],
    "hits": ["@bar5"]}

spec = {"size": "16:9", "fps": 30,
        "compositions": {**COMPS, "printing": {"layers": [
            {"text": "ATELIER VAUCLAIR", "font": FONT, "weight": 600, "size": 46, "letterSpacing": "0.14em", "x": 960, "y": 352, "color": "#000"},
            {"text": "CALIBRE 9", "font": FONT, "weight": 500, "size": 36, "letterSpacing": "0.3em", "x": 960, "y": 742, "color": "#000"}]}},
        "music": music,
        "sound": {"design": "off", "space": "room", "ambience": {"type": "room", "level": -12}, "cues": cues},
        "scenes": scenes}
json.dump(spec, open(OUT, "w"), indent=1)
print("wrote", OUT)
