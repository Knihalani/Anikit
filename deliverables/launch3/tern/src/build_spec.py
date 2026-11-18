# Writes spec.json (kept as source; the spec is the film).
import json
F = "Bricolage Grotesque"
def txt(s, size, y="50%", extra=None):
    d = {"text": s, "font": F, "weight": 800, "size": size, "color": "#FFFFFF", "x": "50%", "y": y, "anchor": "center", "lineHeight": 0.95}
    if extra: d.update(extra)
    return d
def room(u, chan=None, extra=None):
    L = {"type": "shader", "src": "room.glsl", "uniforms": u, "supersample": False}
    if chan: L["channels"] = [{"layers": chan}]
    if extra: L.update(extra)
    return L

scenes = []
# S1 — hook: a pluck's ring already crossing the shaft
scenes.append({"duration": 2.5, "layers": [room({
    "zoom": "=1.25 + 0.06*t", "camx": "=-0.12 + 0.01*t", "camy": 0.12,
    "sx": -0.55, "sy": 0.32, "w0": -0.45, "w1": 0.80, "amp": 1.8, "decay": 0.7})]})
# S2 — low over the boards; bird; "Some sounds happen once."
scenes.append({"duration": 2.5, "layers": [room({
    "zoom": 0.95, "camx": "=0.02 + 0.035*t", "camy": -0.12,
    "sx": 0.55, "sy": 0.30, "w0": 0.25, "w1": 1.47, "amp": 1.7, "decay": 0.8, "bird": 0.05, "ink": 1.0, "tscale": 1.0},
    [txt("Some sounds\nhappen once.", "12vh", "50%", {"enter": {"preset": "fadeIn", "duration": 0.01}})])]})
# S3 — the recorder rises into the sun; the pip comes on
scenes.append({"duration": 2.5, "layers": [room({
    "zoom": 3.1, "camx": "=-0.33 + 0.004*t", "camy": "=-0.06 - 0.5*pow(1-smoothstep(0,1.4,t),3)*0.8",
    "devx": -0.35, "devy": "=-0.08 - 0.95*pow(1-smoothstep(0,1.35,t),2.2)", "devs": 1.0,
    "devsun": "=0.25 + 0.75*smoothstep(0.5,1.3,t)", "pip": "=smoothstep(1.35,1.45,t)",
    "sx": -0.2, "sy": 0.1})]})
# S4 — the chord's ring stops and hangs; "Already listening."
scenes.append({"duration": 2.5, "layers": [room({
    "zoom": "=1.0 + 0.025*t", "camx": 0.04, "camy": 0.0,
    "devx": "=-0.44 + 0.003*sin(t*1.3)", "devy": "=-0.08 + 0.004*sin(t*0.9)", "devs": 1.0, "pip": 1.0,
    "sx": 0.0, "sy": 0.0, "w0": 0.0, "amp": 1.5, "decay": 0.5, "hold": 1.0, "ink": 1.0, "tscale": 1.0},
    [txt("Already\nlistening.", "15vh", "50%", {"start": 0.9, "enter": {"preset": "fadeIn", "duration": 0.15}})])]})
# S5 — the hit: rings bigger than the frame, held; pull back until whole
scenes.append({"duration": 3.75, "layers": [room({
    "zoom": "=mix(2.3, 0.5, smoothstep(0.5, 3.4, t))", "camx": "=mix(0.0, 0.0, t)", "camy": "=mix(-0.02, 0.05, smoothstep(0.5,3.4,t))",
    "devx": -0.44, "devy": -0.08, "devs": 1.0, "pip": 1.0,
    "sx": 0.0, "sy": -0.02, "w0": 0.0, "w1": 0.31, "w2": 0.625, "amp": 2.0, "decay": 0.35, "hold": 1.6})]})
# S6 — stop: the room full of kept sound, silence
scenes.append({"duration": 1.25, "layers": [room({
    "zoom": "=0.62 + 0.02*t", "camx": "=0.1 - 0.03*t", "camy": 0.02,
    "devx": -0.44, "devy": -0.08, "devs": 1.0, "pip": 1.0,
    "sx": 0.0, "sy": -0.02, "w0": -1.6, "w1": -1.29, "w2": -0.975, "amp": 2.0, "decay": 0.35, "hold": 0.0})]})
# S7 — the window light becomes the name
scenes.append({"duration": 5.0, "layers": [room({
    "zoom": "=0.86 + 0.035*t", "camx": 0.12, "camy": 0.02,
    "devx": -0.66, "devy": -0.205, "devs": 1.0, "pip": 1.0,
    "sx": 0.0, "sy": -0.02, "w0": -1.6, "w1": -1.29, "w2": -0.975, "amp": 1.4, "decay": 0.35, "hold": 0.0,
    "name": "=smoothstep(0.35, 1.1, t)", "tscale": 1.25, "warm": "=0.3 + 0.12*t"},
    [txt("Tern", "40vh", "40%"),
     txt("Lift to record.", "10vh", "80%", {"start": 2.45, "enter": {"preset": "fadeIn", "duration": 0.3}})])]})

music = {"bpm": 96, "key": "D", "end": "ring", "master": {"reverb": "hall"}, "followCuts": False, "tracks": [
    {"inst": "nylon-guitar", "gain": -2, "reverb": 0.35, "pan": -0.3,
     "notes": "D4/4! r/4 A3/4 r/4 | r/2 r/4 F#4/4 | r/1 | r/1 | r/1 | r/1 | r/1 | r/1"},
    {"inst": "flute", "gain": -5, "reverb": 0.5, "pan": 0.4,
     "notes": "r/1 | r/4 F#6/16 A6/16 F#6/16 r/16 E6/16 F#6/16 r/8 r/2 | r/1 | r/1 | r/1 | r/1 | r/1 | r/1"},
    {"inst": "piano", "gain": -1, "reverb": 0.45,
     "notes": "r/1 | r/1 | r/2 A5/4? r/4 | [D3 A3 F#4 A4 D5]/1 | [D2 D3 A3 B3 F#4]/1 | _/2 r/2 | [D3 A3 E4 F#4 A4]/2 r/8 D5/8 F#5/8 A5/8 | [D3 A3 D4 F#4 A4 D5]/1"},
    {"inst": "strings", "gain": -6, "reverb": 0.5,
     "notes": "r/1 | r/1 | r/1 | r/8 [D4 A4 F#5]/2.. | [B2 F#3 D4 F#4]/1 | _/2 r/2 | [D3 A3 F#4 A4]/1 | [D3 A3 F#4 D5]/1"},
    {"inst": "cello", "gain": -4, "reverb": 0.4,
     "notes": "r/1 | r/1 | r/1 | r/1 | D2/1 | _/2 r/2 | D2/1 | _/1"},
    {"inst": "timpani", "gain": -1,
     "notes": "r/1 | r/1 | r/1 | r/1 | D2/8! D2/8 D2/4! r/2 | r/1 | r/1 | r/1"},
    {"inst": "harp", "gain": -4, "reverb": 0.5, "pan": 0.2,
     "notes": "r/1 | r/1 | r/1 | r/1 | r/1 | r/1 | D4/16 F#4/16 A4/16 D5/16 F#5/16 A5/16 D6/8 r/2 | r/1"}
]}

spec = {"size": "16:9", "fps": 30, "background": "#000000", "music": music,
        "sound": {"design": "off", "ambience": {"type": "room", "level": -8}, "space": "room",
                  "cues": [{"sfx": "cloth", "at": 5.25, "gain": -6}, {"sfx": "click", "at": 6.38, "gain": -10, "x": -0.2},
                           {"sfx": "air", "at": 15.4, "gain": -10}]},
        "scenes": scenes}
json.dump(spec, open("spec.json", "w"), indent=1)
