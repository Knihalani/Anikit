# Pass 2: the pocket. Writes spec.json.
import json
F = "Bricolage Grotesque"
def ocean(u, tr=None):
    L = {"shader": "ocean", "uniforms": u, "supersample": False}
    if tr: L.update(tr)
    return L
def pocket(u, world=None, label=None):
    ch0 = {"layers": label} if label else "noise"
    ch1 = {"layers": [world]} if world else {"layers": [{"shape": "rect", "width": 4, "height": 4, "fill": "#000"}]}
    return {"type": "shader", "src": "pocket.glsl", "uniforms": u, "supersample": False, "channels": [ch0, ch1]}

S = []
# S1 — frame 0: a slit of sea and sun through the pocket mouth, walking, muffled
S.append({"duration": 2.5, "layers": [pocket(
    {"open": "=0.10 + 0.025*sin(t*3.4)", "mw": 1.15, "cx": 0.05, "cy": 0.06, "ang": -4},
    ocean({"sun": 0.85, "swell": 1.5}, {"y": "=540 + 20*sin(t*3.4)"}))]})
# S2 — the mouth gapes once at another angle: a wave goes past, then it's gone
S.append({"duration": 2.5, "layers": [pocket(
    {"open": "=0.03 + 0.30*pow(sin(clamp((t-0.3)/1.7,0,1)*3.14159),1.5)", "mw": 1.25, "cx": -0.1, "cy": -0.12, "ang": 11},
    ocean({"sun": 0.7, "swell": 2.0, "chop": 1.3}, {"scale": 1.8, "y": "45%", "rotation": -9}))]})
# S3 — the lift: the mouth tears open past the frame, the sea floods in
S.append({"duration": 2.5, "layers": [pocket(
    {"open": "=0.05 + 2.6*pow(smoothstep(0.35, 1.7, t), 2.2)", "mw": "=1.0 + 3.0*smoothstep(0.35,1.7,t)", "cx": 0.0, "cy": "=0.15 - 0.2*smoothstep(0.35,1.7,t)",
     "ang": "=-14*(1-smoothstep(0.35,1.9,t))", "sway": "=1-smoothstep(0.3,1.0,t)"},
    ocean({"sun": 0.85, "swell": 1.5}, {"scale": "=1.35 - 0.3*smoothstep(0.35,2.2,t)", "rotation": "=7*(1-smoothstep(0.35,2.2,t))", "y": "=560 - 60*smoothstep(0.35,2.2,t)"}))]})
# S4 — macro on the glitter path, everything crisp
S.append({"duration": 2.5, "layers": [ocean({"sun": 0.9, "swell": 1.2, "chop": 1.4}, {"scale": "=2.7 + 0.08*t", "y": "=40 + 6*t", "x": "=960 - 10*t"})]})
# S5 — wide: the whole sea to the horizon, sun on the line, slow push
S.append({"duration": 2.5, "layers": [ocean({"sun": 0.97, "swell": 2.2, "chop": 1.0}, {"scale": "=1.45 + 0.04*t", "y": "=760 - 10*t"})]})
# S6 — low into a crest, tilted, the loudest moment
S.append({"duration": 1.25, "layers": [ocean({"sun": 0.8, "swell": 2.8, "chop": 1.6, "speed": 1.3}, {"scale": "=3.4 + 0.3*t", "y": "=-260", "rotation": "=-6 + 3*t"})]})
# S7 — stop and end: snapped back into the pocket; the woven label under the slit of light
label = [
    {"text": "Tern", "font": F, "weight": 800, "size": "44vh", "color": "#FFFFFF", "x": "50%", "y": "33%", "anchor": "center"},
    {"text": "Lift to record.", "font": F, "weight": 700, "size": "15vh", "color": "#FFFFFF", "x": "50%", "y": "71%", "anchor": "center",
     "start": 2.6, "enter": {"preset": "fadeIn", "duration": 0.35}},
    {"text": "4 mics  ·  32-bit float  ·  30 hours", "font": F, "weight": 600, "size": "8.5vh", "color": "#FFFFFF", "x": "50%", "y": "88%", "anchor": "center",
     "start": 2.6, "enter": {"preset": "fadeIn", "duration": 0.35}}]
S.append({"duration": 6.25, "layers": [pocket(
    {"open": "=t < 1.25 ? 0.012 : 0.012 + 0.07*smoothstep(1.25, 2.2, t) + 0.006*sin(t*2.0)", "mw": 1.3, "cx": 0.0, "cy": 0.43, "ang": 1.5, "sway": "=0.25*smoothstep(1.25,2.5,t)",
     "lab": 1.0, "labx": 0.0, "laby": "=-0.11 + 0.008*t", "labs": "=1.62 - 0.025*t", "warm": 0.5},
    ocean({"sun": 0.92}), label)]})

surf_def = {"synth": {"osc": [{"wave": "sine", "level": 0.0}], "noise": 1.0, "filter": {"type": "lp", "cutoff": 900, "res": 0.1}, "amp": [0.9, 0.6, 0.6, 1.6]}}
muff = [[0, 450], [8.4, 450], [9.6, 16000], [21.9, 16000], [22.0, 300], [24, 300], [28, 900]]
music = {"bpm": 96, "key": "D", "end": "ring", "followCuts": False, "master": {"reverb": "hall"},
  "define": {"surf": surf_def},
  "tracks": [
    {"inst": "surf", "gain": -10, "notes": "D3/1 | D3/2 D3/2 | D3/1 | D3/2 D3/2 | D3/2 D3/4 D3/4 | D3/2! r/2 | r/1 | r/1", "automate": {"lp": muff}},
    {"inst": "piano", "gain": -2, "reverb": 0.4, "automate": {"lp": [[0, 700], [8.4, 700], [9.6, 18000]]},
     "notes": "r/4 A4/4 F#4/4 D5/4 | r/4 A4/4 E4/4 C#5/4 | r/4 [D3 A3 F#4 A4]/2. | [B2 F#3 D4 F#4 B4]/1 | [G2 D3 B3 D4 G4]/1 | [A2 E3 C#4 E4 A4]/2 r/2 | r/4 D5/8 F#5/8 A5/4 F#5/4 | [D3 A3 D4 F#4 A4 D5]/1"},
    {"inst": "strings", "gain": -5, "reverb": 0.5,
     "notes": "r/1 | r/1 | r/4 [D4 A4 F#5]/2. | [B3 F#4 D5]/1 | [G3 D4 B4]/1 | [A3 E4 C#5]/2 r/2 | [D3 A3 F#4]/1 | [D3 A3 F#4 A4]/1"},
    {"inst": "cello", "gain": -4, "reverb": 0.4, "notes": "r/1 | r/1 | r/4 D3/2. | B2/1 | G2/1 | A2/2 r/2 | D2/1 | _/1"},
    {"inst": "flute", "gain": -6, "reverb": 0.55, "pan": 0.45, "notes": "r/1 | r/1 | r/1 | r/2 A6/16 F#6/16 r/8 A6/16 E6/16 r/8 | r/1 | r/1 | r/1 | r/1"},
    {"inst": "timpani", "gain": -2, "notes": "r/1 | r/1 | r/1 | r/1 | r/2 D2/16 D2/16 D2/16 D2/16 D2/16 D2/16 D2/16 D2/16 | D2/4! r/4 r/2 | r/1 | r/1"},
    {"inst": "harp", "gain": -4, "reverb": 0.5, "pan": 0.2, "notes": "r/1 | r/1 | r/1 | r/1 | r/1 | r/1 | D4/16 F#4/16 A4/16 D5/16 F#5/16 A5/16 D6/8 r/2 | r/1"}
]}
spec = {"size": "16:9", "fps": 30, "background": "#000000", "music": music,
        "sound": {"design": "off", "space": "outdoor",
                  "cues": [{"sfx": "cloth", "at": 0.9, "gain": -12}, {"sfx": "cloth", "at": 3.6, "gain": -12},
                           {"sfx": "cloth", "at": 5.3, "gain": -4}, {"sfx": "whoosh-air", "at": 5.9, "gain": -8},
                           {"sfx": "impact", "at": 12.5, "gain": -6},
                           {"sfx": "cloth", "at": 13.75, "gain": -3}]},
        "scenes": S}
json.dump(spec, open("spec.json", "w"), indent=1)
