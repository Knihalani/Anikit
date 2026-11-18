"""Embeds world.rhai / camera.rhai into spec.json (only to avoid hand-escaping the scripts)."""
import json, pathlib

here = pathlib.Path(__file__).parent
world = (here / "world.rhai").read_text().splitlines()
camera = (here / "camera.rhai").read_text()

INK = "#F3EEDF"
HEAD = {"font": "Archivo", "axes": {"wdth": 100}, "weight": 800, "letterSpacing": "-0.02em", "color": INK}

layers = [
    {"id": "world", "x": 960, "y": 540, "update": camera, "layers": [{"script": world}]},
    # act 2 line
    dict(HEAD, text="One call. Settled instantly.", size=84, x=140, y=150, anchor="topLeft",
         start=4.6, end=7.15, enter={"preset": "mask by line up", "duration": 0.5},
         exit={"preset": "fadeOut", "duration": 0.25}),
    # act 3 line
    {"shape": "rect", "width": 1920, "height": 300, "x": 960, "y": 1080, "anchor": "bottom",
     "fill": "linear-gradient(180deg, #0D100E00, #0D100EEE)", "start": 7.9, "end": 11.15,
     "enter": {"preset": "fadeIn", "duration": 0.4}, "exit": {"preset": "fadeOut", "duration": 0.2}},
    dict(HEAD, text="A webhook at every state change.", size=84, x=140, y=930, anchor="bottomLeft",
         start=8.15, end=11.15, enter={"preset": "mask by line up", "duration": 0.5},
         exit={"preset": "fadeOut", "duration": 0.2}),
    # end card
    dict(HEAD, text="Railyard", size=210, x=140, y=400, anchor="bottomLeft", start=12.15,
         enter={"preset": "mask by line up", "duration": 0.6}),
    {"text": "One API call moves money between bank accounts.", "font": "Archivo",
     "axes": {"wdth": 100}, "weight": 500, "size": 42, "color": "#CFC9B8", "x": 140, "y": 850,
     "anchor": "topLeft", "start": 12.6, "enter": {"preset": "fadeUp", "duration": 0.5}},
]

T = 0.46875  # beat at 128 bpm
j = [1.875 + 1.40625 * (x - 140) / 1640 for x in (540, 1010, 1480)]
cues = [
    {"sfx": "click", "at": 0.47, "x": -0.6, "gain": 0},
    {"sfx": "metal", "at": 0.95, "x": -0.4, "gain": -8, "pitch": 7},
    {"sfx": "passby", "at": 2.6, "x": 0.0, "gain": -6, "duration": 1.4},
    {"sfx": "metal", "at": round(j[0], 3), "x": -0.4, "gain": -4},
    {"sfx": "metal", "at": round(j[1], 3), "x": 0.0, "gain": -4},
    {"sfx": "metal", "at": round(j[2], 3), "x": 0.4, "gain": -4},
    {"sfx": "ping", "at": round(j[0] + 0.2, 3), "x": -0.3, "gain": -6},
    {"sfx": "ping", "at": round(j[1] + 0.2, 3), "x": 0.1, "gain": -6, "pitch": 3},
    {"sfx": "ping", "at": round(j[2] + 0.2, 3), "x": 0.5, "gain": -6, "pitch": 7},
    {"sfx": "thump", "at": 3.28, "x": 0.6, "gain": -4},
    {"sfx": "whoosh-deep", "at": 8.2, "gain": -8},
    {"sfx": "ticker", "at": 9.2, "gain": -14, "duration": 2.0},
    {"sfx": "whoosh", "at": 11.9, "gain": -6},
    {"sfx": "impact", "at": 12.25, "gain": -6},
]

music = {
    "bpm": 128, "key": "Dm", "length": "15s", "seed": 3, "end": "ring",
    "tracks": [
        # clockwork marimba ostinato: the panel's relays
        {"inst": "marimba", "gain": -5, "pan": -0.2, "reverb": 0.2,
         "notes": "o4 l16 (D4 A4 F4 A4 D5 A4 F4 A4 D4 A4 F4 A4 E5 A4 F4 A4)x2 "
                  "(D4 Bb4 F4 Bb4 D5 Bb4 F4 Bb4 D4 Bb4 F4 Bb4 F5 Bb4 F4 Bb4) "
                  "(E4 C5 G4 C5 E5 C5 G4 C5 E4 C5 G4 C5 G5 C5 G4 C5) "
                  "(D4 A4 F4 A4 D5 A4 F4 A4 D4 A4 F4 A4 E5 A4 F4 A4) "
                  "(D4 Bb4 F4 Bb4 D5 Bb4 F4 Bb4 D4 Bb4 F4 Bb4 F5 Bb4 F4 Bb4) "
                  "[D4 F4 A4 D5]/1 | r1"},
        # the train: clickety-clack
        {"inst": "drums", "at": "@bar2", "bars": 5, "gain": -3,
         "drums": {"kick": "x.....x.x.....x.", "rim": "..x...x...x...x.", "shaker": "xxxxxxxxxxxxxxxx",
                   "crash": "x..............."}},
        {"inst": "pick-bass", "at": "@bar2", "gain": -4,
         "notes": "o2 l8 D2 D2 D3 D2 D2 D2 D3 D2 | Bb1 Bb1 Bb2 Bb1 C2 C2 C3 C2 | D2 D2 D3 D2 D2 D2 D3 D2 | "
                  "Bb1 Bb1 Bb2 Bb1 Bb1 Bb1 Bb2 Bb1 | C2 C2 C3 C2 C2 C2 C3 C2 | D2/1"},
        {"inst": "strings", "at": "@bar4", "gain": -5, "reverb": 0.45,
         "notes": "o4 [D4 F4 A4]/1 | [D4 F4 A4 D5]/1 | [Bb3 D4 F4 Bb4]/1 | [C4 E4 G4 C5]/1 | [D4 F4 A4 D5]/1"},
        {"inst": "tubular-bells", "at": "@bar7", "gain": -6, "reverb": 0.5, "notes": "o5 D5/2 A4/2 | D5/1"},
        {"inst": "pad-warm", "at": "@bar7", "gain": -10, "notes": "o3 [D3 A3 D4 F4]/1 | _/1"},
    ],
    "sfx": [{"sfx": "riser", "at": "@bar5", "gain": -8}, {"sfx": "riser", "at": "@bar7", "gain": -10}],
}

spec = {
    "size": "16:9", "fps": 30,
    "intent": {"pace": "medium"},
    "background": "#121512",
    "post": {"bloom": 0.22, "bloomThreshold": 0.7, "halation": 0.15, "vignette": 0.35, "grain": 0.025,
             "contrast": 1.05},
    "motionBlur": {"shutter": 180},
    "music": music,
    "sound": {"design": "off", "space": "room", "cues": cues},
    "scenes": [{"duration": 15, "background": "#121512", "layers": layers}],
}
(here / "spec.json").write_text(json.dumps(spec, indent=1))
print("ok")
