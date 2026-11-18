#!/usr/bin/env python3
import json, os
HERE = os.path.dirname(os.path.abspath(__file__))
BPM = 124.0
PREWARM = float(os.environ.get("PREWARM", "0"))
DUR = 10.0
hdr = f"#define BPM {BPM:.1f}\n#define PRE {PREWARM:.3f}\n"
rd = hdr + open(os.path.join(HERE, "glsl/rd_step.glsl")).read()
view = hdr + open(os.path.join(HERE, "glsl/view.glsl")).read()
organism = {"size": [1080, 1920], "duration": PREWARM + DUR + 0.5,
    "layers": [{"shader": view, "channels": ["D"], "supersample": False,
                "buffers": {"A": {"shader": rd, "channels": ["D"]},
                            "B": {"shader": rd, "channels": ["A"]},
                            "C": {"shader": rd, "channels": ["B"]},
                            "D": {"shader": rd, "channels": ["C"]}}}]}
beat = 60.0 / BPM
LABELS = [
  {"text": "SPECIMEN 07", "font": "JetBrains Mono", "size": 30, "weight": 500, "color": "#E8E4DC", "letterSpacing": "0.18em",
   "x": "7%", "y": "6.5%", "anchor": "left", "start": 16*beat + 0.3, "animate": [{"from": {"opacity": 0}, "duration": 0.4}]},
  {"text": "grown live, on the beat", "font": "JetBrains Mono", "size": 30, "color": "#9B978F", "letterSpacing": "0.04em",
   "x": "7%", "y": "9%", "anchor": "left", "start": 16*beat + 0.45, "animate": [{"from": {"opacity": 0}, "duration": 0.4}]},
  {"text": "{value:,} generations", "value": "=floor(min(gt, 10) * 120)", "font": "JetBrains Mono", "size": 30, "color": "#E8E4DC",
   "x": "93%", "y": "93.5%", "anchor": "right", "start": 16*beat + 0.3, "animate": [{"from": {"opacity": 0}, "duration": 0.4}]},
]
spec = {
  "size": [1080, 1920], "fps": 30, "transition": "cut", "background": "#000",
  "audio": [{"src": "score.wav"}],
  "intent": {"pace": "fast", "feeling": "alive, hypnotic, precise"},
  "theme": {"fonts": {"mono": "JetBrains Mono"}},
  "scenes": [{"duration": DUR, "layers": organism["layers"] + LABELS}],
}
json.dump(spec, open(os.path.join(HERE, "bloom.json"), "w"), indent=1)
print("ok")
