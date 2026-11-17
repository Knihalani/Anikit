#!/usr/bin/env python3
"""Generates substation.json: a 15 s 9:16 line-up reveal for Substation.

Concept: the decommissioned power station is switched back on. Each artist is a
circuit that goes live on the beat (an arc of current, a breaker label, a kV
readout); the run peaks with all eight breakers closed, then the station name.
Music: hand-written techno at 128 BPM, so 8 bars = 15 s and every cut is on the grid.
"""
import json, os

BEAT = 60 / 128
BAR = 4 * BEAT
YEL, INK, WHITE, ARC = "#FFD400", "#0B0B0C", "#F2F2EE", "#9FD8FF"
HEAVY = "Big Shoulders"
MONO = "JetBrains Mono"

# Arc of current: a jagged bolt that re-strikes ~18 times a second.
ARC_GLSL = r"""
uniform float seed; // = 0.0
uniform float amount; // = 1.0
uniform float vertical; // = 1.0
float h1(float n){ return fract(sin(n*12.9898)*43758.5453); }
float nz(float x, float s){ float i=floor(x); float f=fract(x); f=f*f*(3.0-2.0*f); return mix(h1(i+s*7.1), h1(i+1.0+s*7.1), f)*2.0-1.0; }
void mainImage(out vec4 o, in vec2 fc){
  vec2 uv = fc / iResolution.xy;
  float along = vertical > 0.5 ? uv.y : uv.x;
  float across = vertical > 0.5 ? uv.x : uv.y;
  float span = vertical > 0.5 ? iResolution.x : iResolution.y;
  float k = floor(iTime*18.0) + seed*31.0;
  float c0 = 0.5 + 0.22*nz(along*2.5, k) + 0.08*nz(along*9.0, k+3.0) + 0.035*nz(along*31.0, k+9.0);
  float c1 = c0 + 0.06*nz(along*6.0, k+17.0) + 0.03*nz(along*40.0, k+23.0);
  float d0 = abs(across-c0)*span;
  float d1 = abs(across-c1)*span;
  float core = exp(-d0*d0/6.0) + 0.6*exp(-d1*d1/3.0)*step(0.4, h1(k+2.0));
  float glow = exp(-d0/55.0)*0.55 + exp(-d0/220.0)*0.25;
  float flick = 0.35 + 0.65*step(0.22, h1(k+1.0));
  vec3 col = vec3(0.92,0.97,1.0)*core*1.6 + vec3(0.35,0.62,1.0)*glow;
  col *= flick*amount;
  o = vec4(col, 1.0);
}
"""


def arc(seed, vertical=1, amount=1.0, start=None, end=None, opacity=1):
    layer = {"shader": ARC_GLSL, "uniforms": {"seed": seed, "amount": amount, "vertical": vertical},
             "blend": "add", "opacity": opacity}
    if start is not None:
        layer["start"] = start
    if end is not None:
        layer["end"] = end
    return layer


def slam(dur=0.12):
    return [{"from": {"scale": 1.9, "opacity": 0, "blur": 16}, "duration": dur, "ease": "outExpo"}]


def cam(rot, zoom=1.22, shake=12):
    return {"animate": [{"from": {"zoom": zoom, "rotation": rot}, "to": {"zoom": 1, "rotation": 0},
                         "duration": 0.24, "ease": "outExpo"},
                        {"to": {"zoom": 1.06}, "at": 0.24, "duration": 0.7, "ease": "linear"}],
            "shake": {"amount": shake, "at": 0, "duration": 0.16}}


def hazard(y, color=INK, h=120, start=0):
    """A band of diagonal hazard stripes, scrolling."""
    return {"layers": [{"shape": "rect", "width": 46, "height": 260, "fill": color, "repeat": 34,
                        "x": "=-200 + $i*92 + (t*260 % 92)", "y": 0, "rotation": 35, "anchor": "center"}],
            "x": 0, "y": y, "width": 1080, "height": h, "anchor": "left", "clip": True}


def label(text, y, color, x="7%", size=34, anchor="left", start=0.0):
    return {"text": text, "font": MONO, "size": size, "weight": 700, "color": color, "x": x, "y": y,
            "anchor": anchor, "letterSpacing": "0.08em", "start": start,
            "animate": [{"from": {"opacity": 0, "x": "-30"}, "duration": 0.1, "ease": "outExpo"}]}


def name_lines(lines, y0, color, step):
    out = []
    for j, ln in enumerate(lines):
        out.append({"text": ln, "font": HEAVY, "size": 470, "fit": True, "width": 900, "height": 470,
                    "color": color, "uppercase": True, "lineHeight": 0.82, "letterSpacing": "-0.01em",
                    "x": "50%", "y": y0 + j * step, "anchor": "center",
                    "animate": [{"from": {"scale": 1.9, "opacity": 0, "blur": 16, "y": f"{'+' if j else '-'}160"},
                                 "at": 0.03 * j, "duration": 0.13, "ease": "outExpo"}]})
    return out


ARTISTS = [["Ilka", "Varn"], ["DJ", "Moth"], ["Petra", "Sólveig"], ["Kassim", "Ade"],
           ["Nullform"], ["Rhea", "Okoro"], ["Tomas", "Bell"], ["Veda", "K"]]

scenes = []

# Bar 1: the hook. "DECOMMISSIONED" (struck out) / "POWER STATION" / "RE-ENERGISED."
scenes.append({"duration": BEAT, "background": INK, "camera": cam(3), "layers": [
    hazard(260, YEL, 90), hazard(1660, YEL, 90),
    {"text": "DECOMM-", "font": HEAVY, "size": 330, "fit": True, "width": 900, "height": 330, "color": WHITE,
     "x": "50%", "y": 820, "animate": slam()},
    {"text": "ISSIONED", "font": HEAVY, "size": 330, "fit": True, "width": 900, "height": 330, "color": WHITE,
     "x": "50%", "y": 1110, "animate": slam()},
    {"shape": "rect", "width": 1000, "height": 44, "fill": "#FF3B1F", "x": "50%", "y": 975, "rotation": -9,
     "animate": [{"from": {"scaleX": 0}, "to": {"scaleX": 1}, "at": 0.12, "duration": 0.14, "ease": "outExpo"}]},
]})
scenes.append({"duration": BEAT, "background": YEL, "camera": cam(-3), "layers": [
    {"text": "POWER\nSTATION", "font": HEAVY, "size": 330, "color": INK, "lineHeight": 0.84,
     "align": "center", "x": "50%", "y": "47%", "animate": slam()},
    label("GRID REF. SUB-01 · OFFLINE SINCE 1991", 1480, INK, x="50%", anchor="center", size=30),
]})
scenes.append({"duration": 2 * BEAT, "background": INK,
               "camera": {"animate": [{"from": {"zoom": 1.35}, "to": {"zoom": 1}, "duration": 0.3, "ease": "outExpo"}],
                          "shake": {"amount": 22, "at": 0, "duration": 0.35}}, "layers": [
    {"shape": "rect", "width": 1080, "height": 1920, "fill": WHITE, "x": "50%", "y": "50%",
     "animate": [{"from": {"opacity": 1}, "to": {"opacity": 0}, "duration": 0.18, "ease": "outCubic"}]},
    arc(1.0, 1, 1.2),
    {"text": "RE-\nENERG-\nISED.", "font": HEAVY, "size": 380, "color": WHITE, "lineHeight": 0.8,
     "x": "7%", "y": "50%", "anchor": "left", "animate": slam(0.14)},
    label("⚡ 14—15 MARCH · TWO NIGHTS", 1480, YEL, size=34),
]})

# Bars 2–5: eight circuits go live, two beats each.
for n, lines in enumerate(ARTISTS):
    dark = n % 2 == 0
    bg, fg, accent = (INK, WHITE, YEL) if dark else (YEL, INK, INK)
    top = n % 4 in (0, 3)
    step = 400
    if len(lines) == 1:
        y0 = 960
    else:
        y0 = 720 if top else 830
    layers = []
    if dark:
        layers.append(arc(2.0 + n, n % 3 != 1, 0.9))
    else:
        layers += [hazard(110, INK, 70), hazard(1810, INK, 70)]
    if not dark:
        layers.append({"text": f"{n + 1:02d}", "font": HEAVY, "size": 1500, "color": "#F0C400", "x": "62%",
                       "y": "52%", "anchor": "center", "rotation": -8})
    layers += name_lines(lines, y0, fg, step)
    kv = 11 + n * 4
    layers += [
        label(f"CIRCUIT {n + 1:02d}/08", 270, accent, size=40),
        {"text": "● LIVE", "font": MONO, "size": 40, "weight": 700, "x": "93%", "y": 270,
         "anchor": "right", "color": "#FF3B1F" if dark else "#B3150A", "opacity": "=fract(t*4) < 0.5 ? 1 : 0.55"},
        {"text": "{value} kV", "value": f"=floor({kv} * min(1, t*6) + 0.9*sin(t*40))", "font": MONO, "size": 40,
         "weight": 700, "color": accent, "x": "7%", "y": 1480, "anchor": "left"},
        {"shape": "rect", "height": 14, "width": 560, "fill": accent, "x": "93%", "y": 1480,
         "anchor": "right", "animate": [{"from": {"scaleX": 0}, "to": {"scaleX": 1}, "duration": 0.35, "ease": "outExpo"}]},
    ]
    scenes.append({"duration": 2 * BEAT, "background": bg, "camera": cam(4 if n % 2 else -4), "layers": layers})

# Bar 6: the peak. All eight breakers close, one per 16th note.
rows = []
full = [" ".join(a) for a in ARTISTS]
for i, nm in enumerate(full):
    y = 470 + i * 150
    at = i * BEAT / 4
    rows += [
        {"text": f"{i + 1:02d}", "font": MONO, "size": 36, "weight": 700, "color": YEL, "x": 70, "y": y,
         "anchor": "left", "start": at},
        {"text": nm, "font": HEAVY, "size": 132, "color": WHITE, "uppercase": True, "x": 160, "y": y,
         "anchor": "left", "start": at,
         "animate": [{"from": {"x": "+140", "opacity": 0}, "duration": 0.12, "ease": "outExpo"}]},
        {"shape": "rect", "width": 1000, "height": 3, "fill": "#3A3A36", "x": "50%", "y": y + 72, "start": at},
        {"shape": "circle", "width": 26, "height": 26, "x": 1010, "y": y, "start": at,
         "fill": "#FF3B1F", "glow": "#FF3B1F"},
    ]
scenes.append({"duration": BAR, "background": INK,
               "camera": {"animate": [{"from": {"zoom": 1.15, "y": 120}, "to": {"zoom": 1, "y": 0},
                                       "duration": BAR, "ease": "outCubic"}],
                          "shake": {"amount": 6, "at": 0, "duration": 0.2}}, "layers": [
    arc(40.0, 0, 0.5, opacity=0.8),
    hazard(150, YEL, 80),
    {"text": "ALL CIRCUITS LIVE", "font": HEAVY, "size": 150, "color": YEL, "x": "50%", "y": 300,
     "anchor": "center", "animate": slam()},
    *rows,
]})

# Bars 7–8: the station name, then the held end card.
scenes.append({"duration": 2 * BEAT, "background": YEL, "camera": cam(0, 1.4, 26), "layers": [
    {"text": "SUB", "font": HEAVY, "size": 800, "fit": True, "width": 900, "height": 760, "color": INK,
     "x": "50%", "y": 720, "animate": slam(0.1)},
    {"text": "STATION", "font": HEAVY, "size": 400, "fit": True, "width": 900, "height": 360, "color": INK,
     "x": "50%", "y": 1300, "animate": [{"from": {"scale": 1.9, "opacity": 0, "blur": 16}, "at": BEAT,
                                           "duration": 0.1, "ease": "outExpo"}]},
]})
END = 15 - (BEAT * 4 + 8 * 2 * BEAT + BAR + 2 * BEAT)
scenes.append({"duration": END, "background": INK,
               "camera": {"animate": [{"from": {"zoom": 1.12}, "to": {"zoom": 1}, "duration": 0.35, "ease": "outExpo"},
                                      {"to": {"zoom": 1.04}, "at": 0.35, "duration": END - 0.35, "ease": "linear"}],
                          "shake": {"amount": 18, "at": 0, "duration": 0.25}}, "layers": [
    arc(77.0, 1, 0.75),
    hazard(70, YEL, 60), hazard(1850, YEL, 60),
    {"text": "SUB", "font": HEAVY, "size": 700, "fit": True, "width": 900, "height": 560, "color": WHITE,
     "x": "50%", "y": 530, "animate": slam(0.1)},
    {"text": "STATION", "font": HEAVY, "size": 380, "fit": True, "width": 900, "height": 300, "color": WHITE,
     "x": "50%", "y": 945, "animate": slam(0.1)},
    {"shape": "rect", "width": 940, "height": 12, "fill": YEL, "x": "50%", "y": 1120,
     "animate": [{"from": {"scaleX": 0}, "to": {"scaleX": 1}, "at": 0.15, "duration": 0.4, "ease": "outExpo"}]},
    {"text": "14—15 MARCH", "font": HEAVY, "size": 210, "fit": True, "width": 900, "height": 200, "color": YEL, "x": "50%", "y": 1240, "anchor": "center",
     "animate": [{"from": {"opacity": 0, "y": "+80", "blur": 10}, "at": BEAT, "duration": 0.14, "ease": "outExpo"}]},
    {"text": "A DECOMMISSIONED POWER STATION", "font": MONO, "size": 36, "weight": 700, "color": WHITE,
     "letterSpacing": "0.06em", "x": "50%", "y": 1395, "anchor": "center",
     "animate": [{"from": {"opacity": 0}, "at": 2 * BEAT, "duration": 0.1}]},
    {"text": "TWO NIGHTS OF TECHNO · 0 DAYLIGHT", "font": MONO, "size": 36, "weight": 700, "color": YEL,
     "letterSpacing": "0.06em", "x": "50%", "y": 1455, "anchor": "center",
     "animate": [{"from": {"opacity": 0}, "at": 2.5 * BEAT, "duration": 0.1}]},
]})

music = {
    "bpm": 128, "key": "Fm", "length": "15s", "seed": 7, "fit": False, "end": "ring",
    "define": {
        "acid": {"synth": {"osc": [{"wave": "saw", "voices": 2, "detune": 8}, {"wave": "square", "pw": 0.4, "level": 0.5}],
                           "filter": {"type": "ladder", "cutoff": 450, "res": 0.72, "drive": 2.4, "env": 2.8},
                           "fenv": [0.002, 0.14, 0.0, 0.08], "amp": [0.002, 0.2, 0.6, 0.06], "mono": True, "glide": 0.035}},
        "stab": {"synth": {"osc": [{"wave": "supersaw", "voices": 5, "detune": 18}],
                           "filter": {"type": "lp", "cutoff": 2600, "res": 0.2, "env": 1.5},
                           "fenv": [0.001, 0.12, 0.1, 0.1], "amp": [0.001, 0.18, 0.0, 0.12]}},
    },
    "tracks": [
        # Bar 1: no kick yet, just a ticking hat and the acid line opening up.
        {"inst": "drums:techno", "drums": {"hat": "x.x.x.x.x.x.xxxx", "rim": "...x..x....x..x."}, "bars": 1},
        # Bars 2–8: four-on-the-floor.
        {"inst": "drums:techno", "at": "@bar2", "bars": 7,
         "drums": {"kick": "x...x...x...x...", "hat": "..x...x...x...x.", "clap": "....x.......x...",
                   "ohh": "......x.......x.", "rim": "x..x..x...x..x.."},
         "fx": [{"type": "comp", "threshold": -12, "ratio": 4}, {"type": "saturate", "drive": 0.25}]},
        {"inst": "acid", "notes": "o2 l16 (F2 F2 F3 F2 Ab2 F2 Eb3 F2^ F2 C3 F2 F2 Bb2 Ab2 F2 Eb2)x8", "duck": 6,
         "automate": {"synth.cutoff": "=350 + 3800*smooth(clamp(bar/6, 0, 1))", "pan": "=0.25*sin(beat*pi/2)"},
         "fx": [{"type": "delay", "note": "1/8d", "feedback": 0.3, "wet": 0.16}, {"type": "saturate", "drive": 0.35}]},
        {"inst": "stab", "at": "@bar2", "notes": "o4 l8 ([F3 Ab3 C4] r r [F3 Ab3 C4] r r [Eb3 G3 Bb3] r)x7", "gain": -7,
         "duck": 5, "fx": [{"type": "reverb", "wet": 0.25}]},
        {"inst": "bass-808", "at": "@bar2", "notes": "o1 l8 (r F1 r F1 r F1 r F1)x7", "gain": -5, "duck": 8},
    ],
    "sfx": [{"sfx": "zap", "at": 2 * BEAT}, {"sfx": "impact", "at": 4 * BEAT}, {"sfx": "riser", "at": "@bar6"},
            {"sfx": "impact", "at": "@bar6"}, {"sfx": "zap", "at": "@bar7"}, {"sfx": "boom", "at": "@bar7+0.9375"}],
    "master": {"lufs": -14, "ceiling": -1},
}

spec = {
    "size": [1080, 1920],
    "fps": 30,
    "intent": {"pace": "frantic", "genre": "music"},
    "fonts": {HEAVY: "fonts/BigShoulders-Bold.ttf"},
    "post": {"bloom": 0.25, "saturation": 1.08, "grain": 0.035, "vignette": 0.2, "aberration": 0.12},
    "music": music,
    "scenes": [dict(s, transition="cut") for s in scenes],
}

here = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(here, "substation.json"), "w") as f:
    json.dump(spec, f, indent=1, ensure_ascii=False)
print("total", round(sum(s["duration"] for s in scenes), 4), "s,", len(scenes), "scenes")
