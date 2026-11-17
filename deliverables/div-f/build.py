"""Generates calibre9.json (the anikit-video spec) for the Atelier Vauclair Calibre 9 teaser.
Timeline (100 bpm, bar = 2.4 s): bar1 macro cut, bar2 pull-out + 38 mm, bar3-4 72-hour time-lapse,
then the 150 ring, bar5 the name."""
import json

S = "#F09A72"  # salmon accent
W = "#EEF1F4"
GREY = "#9AA3AD"
SERIF = "Instrument Serif"
SANS = "Inter Display"


def sm(a, b):
    return f"smoothstep({a}, {b}, gt)"


def kicker(text, start, end, y="62%", color=S):
    return {"text": text, "font": SANS, "size": "2.5vh", "weight": 600, "letterSpacing": "0.32em",
            "uppercase": True, "color": color, "x": "7.5%", "y": y, "anchor": "left",
            "start": start, "end": end, "enter": {"preset": "wipe", "duration": 0.35},
            "exit": {"preset": "fade", "duration": 0.25}}


def head(text, start, end, y="70%", size="10vh"):
    return {"text": text, "font": SERIF, "size": size, "color": W, "letterSpacing": "-0.01em",
            "x": "7.1%", "y": y, "anchor": "left", "start": start, "end": end,
            "enter": "mask by line up", "exit": {"preset": "fade", "duration": 0.25}}


dial = {
    "src": "dial.glsl",
    "uniforms": {
        # camera: macro until 2.5 s, one long pull-out to the full watch, a slow push at the end
        "uK": "=smoothstep(2.4, 3.7, gt) + 0.6*smoothstep(9.6, 12.0, gt)",
        "uLight": f"=0.5 + 0.55*gt + 6.2832*3*{sm(4.9, 7.2)}",
        "uCut": f"={sm(0.15, 2.2)}",
        "uCutA": "=1 - smoothstep(2.6, 3.4, gt)",
        "uHours": f"=10.15 + 72*{sm(4.9, 7.2)}",
        "uSpin": "=72*6*clamp((gt-4.9)/2.3,0,1)*(1-clamp((gt-4.9)/2.3,0,1))/2.3/30",
        "uReserve": f"={sm(4.9, 7.2)}",
        "uResA": "=smoothstep(4.6, 5.0, gt) * (1 - smoothstep(7.9, 8.2, gt))",
        "uRing": "=clamp((gt - 8.1) / 0.7, 0, 1)",
        "uRingA": "=smoothstep(8.0, 8.2, gt) * (1 - 0.0*gt)",
        "uOne": "=smoothstep(8.9, 9.3, gt)",
        "uWX": 1280,
    },
}

# left scrim so type stays readable over the grooves in the macro shot
scrim = {"shape": "rect", "width": 1100, "height": 1080, "x": 0, "y": 0, "anchor": "topLeft",
         "fill": "linear-gradient(90deg, #0B0C0Eee, #0B0C0E00)", "start": 0, "end": 2.8,
         "exit": {"preset": "fade", "duration": 0.4}}

# technical-drawing dimension line under the case at full view (case = 38 mm = 722 px at 19 px/mm)
cx, R = 1280, 19 * 19
yd = 540 + R + 60
dim = {"start": 3.45, "end": 4.95, "enter": "draw", "exit": {"preset": "fade", "duration": 0.25}, "draw": [
    {"line": [cx - R, 540 + 40, cx - R, yd + 18], "stroke": GREY, "width": 1.5},
    {"line": [cx + R, 540 + 40, cx + R, yd + 18], "stroke": GREY, "width": 1.5},
    {"line": [cx - R, yd, cx - 70, yd], "stroke": W, "width": 2},
    {"line": [cx + 70, yd, cx + R, yd], "stroke": W, "width": 2},
    {"points": [[cx - R, yd], [cx - R + 16, yd - 6], [cx - R + 16, yd + 6]], "closed": True, "fill": W},
    {"points": [[cx + R, yd], [cx + R - 16, yd - 6], [cx + R - 16, yd + 6]], "closed": True, "fill": W},
]}
dimlabel = {"text": "38 mm", "font": SANS, "size": "2.6vh", "weight": 500, "letterSpacing": "0.08em",
            "color": W, "x": cx, "y": yd, "start": 3.7, "end": 4.95, "enter": {"preset": "fade", "duration": 0.3},
            "exit": {"preset": "fade", "duration": 0.25}}

counter = {"text": "{value} h", "value": f"=floor(72*{sm(4.9, 7.2)} + 0.0001)", "font": SERIF, "size": "24vh",
           "color": W, "x": "7%", "y": "58%", "anchor": "left", "start": 4.85, "end": 8.0,
           "enter": {"preset": "fade", "duration": 0.2}, "exit": {"preset": "fade", "duration": 0.25},
           "animate": [{"keyframes": {"scale": [1, 1.06, 1]}, "at": 2.35, "duration": 0.45, "ease": "outExpo"}]}

final_rule = {"shape": "rect", "width": 520, "height": 3, "fill": S, "x": "7.5%", "y": "61.5%", "anchor": "left",
              "start": 10.0, "animate": [{"from": {"scaleX": 0}, "to": {"scaleX": 1}, "duration": 0.7,
                                          "ease": "outExpo"}]}

layers = [
    dial,
    scrim,
    kicker("Guilloché dial", 0.35, 2.55, y="60%"),
    head("Hand-finished.", 0.5, 2.55, y="69%", size="11vh"),
    kicker("Steel case", 3.55, 4.95, y="44%"),
    head("38 mm.", 3.7, 4.95, y="53%", size="14vh"),
    dim, dimlabel,
    kicker("Power reserve", 4.95, 8.0, y="40%"),
    counter,
    {"text": "of running, wound once.", "font": SERIF, "size": "5vh", "color": GREY, "x": "7.5%", "y": "72%",
     "anchor": "left", "start": 6.7, "end": 8.0, "enter": "mask by line up", "exit": {"preset": "fade", "duration": 0.25}},
    kicker("Limited run", 8.1, 9.55, y="44%"),
    head("One of 150.", 8.25, 9.55, y="53%", size="12vh"),
    {"text": "Atelier Vauclair", "font": SANS, "size": "2.4vh", "weight": 600, "letterSpacing": "0.42em",
     "uppercase": True, "color": W, "x": "7.5%", "y": "40%", "anchor": "left", "start": 9.75,
     "enter": {"preset": "wipe", "duration": 0.5}},
    {"text": "Calibre 9", "font": SERIF, "size": "17vh", "color": W, "x": "7%", "y": "51%", "anchor": "left",
     "start": 9.6, "enter": "mask by line up"},
    final_rule,
    {"text": "38 mm · 72 h · 150 pieces", "font": SANS, "size": "2.5vh",
     "weight": 500, "letterSpacing": "0.04em", "color": GREY, "x": "7.5%", "y": "67%", "anchor": "left",
     "start": 10.0, "enter": {"preset": "fade", "duration": 0.4}},
]

spec = {
    "size": [1920, 1080],
    "fps": 30,
    "background": "#0B0C0E",
    "intent": {"pace": "calm"},
    "music": {
        "bpm": 100, "key": "Dm", "length": "12s", "end": "ring",
        "hits": ["@bar2", "@bar4", "@bar5"],
        "master": {"reverb": "hall"},
        "tracks": [
            {"inst": "piano", "chords": "Dm Bb F C | Dm Bb F A", "play": "broken", "gain": -3},
            {"inst": "strings", "chords": "Dm Bb F C | Dm Bb F A", "play": "pad", "gain": -8, "at": "@bar2"},
            {"inst": "cello", "chords": "Dm Bb F C | Dm Bb F A", "bass": "root", "gain": -6, "at": "@bar2"},
            {"inst": "drums", "drums": {"rim": "x.x.x.x.x.x.x.x."}, "gain": -16},
            {"inst": "drums", "drums": {"kick": "x...............", "rim": "....x.......x..."}, "gain": -9, "at": "@bar3"},
        ],
        "sfx": [{"sfx": "chime", "at": "@bar5"}],
    },
    "post": {"vignette": 0.15, "grain": 0.0, "contrast": 1.04},
    "scenes": [{"duration": 12, "layers": layers}],
}

with open("calibre9.json", "w") as fh:
    json.dump(spec, fh, indent=1, ensure_ascii=False)
print("wrote calibre9.json")
