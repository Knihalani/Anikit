"""Builds spec.json for 'Low Tide' (Harbour Lights): python3 build.py"""
import json
B = 60 / 84  # seconds per beat
music = json.load(open("music.json"))

FONT = {"font": "Archivo", "axes": {"wdth": 62, "wght": 900}, "uppercase": True, "color": "#FFFFFF", "anchor": "left", "letterSpacing": "0.04em"}

def word(id_, text, beat, scene_start, size, x, y, rise=0.9):
    """A lyric word in the lamp mask: it powers up on its melody note."""
    l = dict(FONT, id=id_, text=text, size=size, x=x, y=y,
             animate=[{"from": {"opacity": 0}, "at": max(0.0, beat * B - scene_start), "duration": rise, "ease": "outCubic"}])
    return l

def row(words, scene_start, size, x0, y, gap):
    out, prev = [], None
    for i, (text, beat) in enumerate(words):
        wid = f"w{abs(hash((text, beat))) % 10**8}"
        x = x0 if prev is None else f"={prev}.right + {gap}"
        out.append(word(wid, text, beat, scene_start, size, x, y))
        prev = wid
    return out

def mask(layers):
    return {"layers": [{"shape": "rect", "width": 1920, "height": 1080, "fill": "#000000"}] + layers}

def shader(scene_start, dur, layers, uniforms, animate=None):
    l = {"shader": None, "src": "lamps.glsl", "channels": [mask(layers)], "uniforms": uniforms}
    del l["shader"]
    if animate:
        l["animate"] = animate
    return l

scenes = []
# 1 · Hold me at low tide — blue hour, tide in, close and low over the water.
s0, d0 = 0.0, 10 * B
beacon = {"shape": "circle", "width": 44, "height": 44, "fill": "#FFFFFF", "x": 1560, "y": 880,
          "opacity": "=t < 0.35 ? 0 : 0.8 + 0.2 * sin(t * 2.6)"}
# The quay lamps switch on along the water, one per piano eighth, before the first word.
quay = [{"shape": "circle", "width": 54, "height": 54, "fill": "#FFFFFF", "x": 130 + 140 * k, "y": 95,
         "animate": [{"from": {"opacity": 0}, "at": 0.15 + k * B / 2, "duration": 0.25, "ease": "outCubic"}]} for k in range(12)]
w1 = quay + row([("Hold", 4), ("me", 5), ("at", 5.5)], s0, 230, 230, 300, 50) + row([("low", 6), ("tide", 7)], s0, 400, 210, 640, 70)
scenes.append({"duration": round(d0, 4), "layers": [shader(s0, d0, w1,
    {"mode": 2, "horizon": 1.2, "pitch": 12, "panx": "=-60 + 120 * t / 7.14", "tideout": 0.0, "warmth": 0.0, "night": 0.0, "zoom": "=1.0 + 0.06 * t / 7.14", "glowk": 1.0, "fade": 1})]})

# 2 · when the harbour lights are gone — night falls, the tide is out, wide over the mud.
s1, d1 = 10 * B, 6.5 * B
beacon2 = dict(beacon, width=80, height=80, x=150, y=730, opacity="=0.8 + 0.2 * sin(t * 2.6)")
w2 = [beacon2] + row([("when", 10), ("the", 10.5), ("harbour", 11)], s1, 190, 230, 430, 46) + row([("lights", 12), ("are", 13), ("gone", 13.5)], s1, 190, 230, 640, 46)
scenes.append({"duration": round(d1, 4), "layers": [shader(s1, d1, w2,
    {"mode": 0, "horizon": 0.25, "pitch": 12, "panx": "=90 - 160 * t / 4.64", "tideout": 1.0, "warmth": 0.15, "night": "=min(0.72, 0.4 + 0.5 * t / 4.6)", "zoom": "=1.12 - 0.08 * t / 4.6",
     "glowk": "=1 + 2.2 * exp(-max(0, t - 1.43) * 3) * (t > 1.43 ? 1 : 0)", "fade": 1,
     "outx": "=t < 2.5 ? 9999 : 2700 - 2650 * smoothstep(2.5, 5.6, t)"})]})

# 3 · we were never meant to stay — cold lamps in the water, seen from above; the tide surges back and takes "stay".
s2, d2 = 16.5 * B, 6 * B
w3 = (row([("we", 16.5), ("were", 17)], s2, 200, 170, 230, 50) + row([("never", 17.5), ("meant", 18.5)], s2, 200, 170, 470, 50)
      + row([("to", 19.5), ("stay", 20)], s2, 300, 170, 760, 60))
scenes.append({"duration": round(d2, 4), "layers": [shader(s2, d2, w3,
    {"mode": 2, "horizon": 1.2, "pitch": 11, "warmth": 0.0, "night": 0.55, "zoom": "=1.1 - 0.08 * t / 4.3", "panx": "=-50 + 100 * t / 4.3",
     "wash": "=-150 + 650 * smoothstep(3.0, 4.2, t)", "glowk": 1.2, "fade": "=t < 3.93 ? 1 : 0.6"})]})

# 4 · but we stayed on — the light comes back amber; every lamp is the word "on", and the camera pulls back.
s3, d3 = 22.5 * B, 5.5 * B
w4 = row([("but", 22.5), ("we", 23), ("stayed", 24)], s3, 210, 470, 210, 46) + [word("on", "on", 25, s3, 560, 610, 600, rise=0.8)]
beacon4 = dict(beacon, opacity=1, x=1500, y=760)
scenes.append({"duration": round(d3, 4), "layers": [shader(s3, d3, w4,
    {"mode": 0, "horizon": 0.2, "pitch": 13, "panx": "=40 * (1 - smoothstep(0, 3.9, t))", "tideout": 0.3, "warmth": "=smoothstep(0.0, 2.2, t)", "night": "=0.8 - 0.25 * smoothstep(1.5, 3.9, t)",
     "zoom": "=1.22 - 0.22 * smoothstep(0.0, 3.9, t)", "glowk": "=1 + 1.6 * exp(-max(0, t - 1.79) * 2) * (t > 1.79 ? 1 : 0)",
     "fade": 1})]})

spec = {
    "size": "16:9", "fps": 30,
    "intent": {"pace": "slow"},
    "music": music,
    "post": {"bloom": 0.55, "bloomThreshold": 0.45, "halation": 0.45, "grain": 0.05, "contrast": 1.04},
    "scenes": scenes,
}
json.dump(spec, open("spec.json", "w"), indent=1)
print("scenes", [s["duration"] for s in scenes], "total", sum(s["duration"] for s in scenes))
