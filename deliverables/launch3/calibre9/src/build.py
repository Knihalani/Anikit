"""Writes spec.json for the Calibre 9 teaser (one shader world, six shots on a 100 bpm grid)."""
import json, os
HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "spec.json")
FONT = "Noto Serif Display"

def dial(dur, **u):
    base = {"zoom": 1.3, "cx": 0, "cy": 0, "rot": 0, "T": 0, "rate": 0, "expo": 1.0, "ink": 1.0, "tone": 0, "sreal": 0}
    base.update(u)
    return {"id": "dial", "type": "shader", "src": "src/dial.glsl",
            "channels": [{"layers": [{"composition": "printing"}]}], "uniforms": base}

def shadow_word(text, size, x, y, start=0.0, rot=0, dur=None):
    l = {"text": text, "font": FONT, "weight": 400, "size": size, "x": x, "y": y, "anchor": "left",
         "color": "#140d08", "blend": "multiply", "rotation": rot, "blur": 2,
         "enter": {"preset": "fade", "delay": start, "duration": 0.5}}
    return l

scenes = []
# 1 · Friday 17:00, macro on the seconds-hand tip crossing 12, real time (bar 1)
scenes.append({"duration": 2.4, "background": "#000", "layers": [
    dial(2.4, zoom="=0.30 - 0.025*t/2.4", cx="=0.66 + 0.04*t/2.4", cy=-0.72, rot=-0.35, T=0.0, sreal="=24.5 + t"),
    {"text": "Friday.", "font": FONT, "weight": 300, "size": "15vh", "x": "60%", "y": "80%", "anchor": "left",
     "color": "#EFE2CC", "enter": {"preset": "fadeUp", "delay": 0.3, "duration": 0.6}}]})
# 2 · the hand centre, time starts to accelerate, sunset → night (bar 2)
scenes.append({"duration": 2.4, "background": "#000", "layers": [
    dial(2.4, zoom="=0.78 - 0.14*t/2.4", cx=0.10, cy=-0.05, rot=0.5,
         T="=0.0007 + 7.5*pow(t/2.4, 3)", rate="=7.5*3*pow(t/2.4, 2)/2.4", sreal=2.4)]})
# 3 · whole dial, Saturday passes, slow rotation (bar 3, first 3 beats)
scenes.append({"duration": 1.8, "background": "#000", "layers": [
    dial(1.8, zoom=1.06, cx=0, cy=0, rot="=-0.30 + 0.25*t/1.8",
         T="=7.5 + 22.5*t/1.8", rate=22.5/1.8)]})
# 4 · baton at 3 with the minute-hand blur; Sunday, then braking into Monday's dark (bar 3 beat 4 → bar 4 beat 2)
scenes.append({"duration": 1.8, "background": "#000", "layers": [
    dial(1.8, zoom="=0.46 - 0.06*t/1.8", cx=0.62, cy=0.12, rot=0.8,
         T="=30 + 30.3*(1 - pow(1 - t/1.8, 3))", rate="=max(30.3*3*pow(1 - t/1.8, 2)/1.8, 0.02)")]})
# 5 · hard stop: the seconds-hand tip alone in the dark, real time again (bar 4 beats 3–4)
scenes.append({"duration": 1.2, "background": "#000", "layers": [
    dial(1.2, zoom=0.17, cx=-0.06, cy=0.80, rot=0.15, T=60.3, sreal="=t", expo=2.2)]})
# 6 · Monday dawn rakes in, hands settle at 8:09, the name appears in the dial printing (bar 5)
scenes.append({"duration": 2.4, "background": "#000", "layers": [
    dial(2.4, zoom="=1.30 + 0.16*(1 - pow(1 - min(t/2.0, 1), 3))", cx=-0.95, cy=0.05, rot=0,
         T="=60.4 + 2.75*(1 - pow(1 - min(t/1.6, 1), 3))",
         rate="=2.75*3*pow(1 - min(t/1.6, 1), 2)/1.6", sreal="=max(t - 1.6, 0)"),
    {"text": "Monday.", "font": FONT, "weight": 300, "size": "15vh", "x": "7%", "y": "44%", "anchor": "left",
     "color": "#EFE6D6", "enter": {"preset": "fadeUp", "delay": 0.55, "duration": 0.6}},
    {"text": "150 pieces.", "font": FONT, "weight": 400, "size": "4.6vh", "x": "7.4%", "y": "58%",
     "anchor": "left", "color": "#D8CDBB", "enter": {"preset": "fade", "delay": 0.75, "duration": 0.5}}]})

# escapement ticks (28,800 vph) in the real-time shots
ticks = [round(k/8, 3) for k in range(0, 19)] + [round(8.4 + k/8, 3) for k in range(0, 10)] + [round(11.2 + k/8, 3) for k in range(0, 6)]
cues = [{"sfx": "tick", "at": t, "gain": -9 if t < 8 else -6, "pitch": 7, "x": 0.1} for t in ticks]

music = {"bpm": 100, "key": "D", "end": "ring", "followCuts": False, "tracks": [
    {"inst": "piano", "gain": -2, "reverb": 0.35, "notes":
        "r/8 A4/8 D5/8 E5/8 F#5/2 | r/8 A4/8 D5/8 E5/8 A5/2 | (D5/8 A4/8 F#5/8 A4/8)x2 | (D5/16 A4/16 G5/16 B4/16)x2 r/2 | [D4 A4 D5 F#5]/1!"},
    {"inst": "strings", "gain": -8, "reverb": 0.5, "notes":
        "r/1 | [D3 A3 F#4]/1 | [B2 F#3 D4]/1 | [G2 D3 B3]/2 r/2 | [D2 A2 F#3 D4]/1"},
    {"inst": "cello", "gain": -9, "reverb": 0.4, "notes": "r/1 | r/1 | B2/1 | G2/2 r/2 | D2/1"}],
    "hits": ["@bar5"]}

spec = {"size": "16:9", "fps": 30,
        "compositions": {"printing": {"layers": [
            {"text": "ATELIER VAUCLAIR", "font": FONT, "weight": 600, "size": 46, "letterSpacing": "0.14em", "x": 960, "y": 352, "color": "#000"},
            {"text": "CALIBRE 9", "font": FONT, "weight": 500, "size": 36, "letterSpacing": "0.3em", "x": 960, "y": 742, "color": "#000"}]}},
        "music": music,
        "sound": {"design": "off", "space": "room", "ambience": {"type": "room", "level": -12}, "cues": cues},
        "scenes": scenes}
json.dump(spec, open(OUT, "w"), indent=1)
print("wrote", OUT)
