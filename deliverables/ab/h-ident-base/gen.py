# Generates spec.json for the Kestrel ident.  python3 gen.py
import json, bird
INK, CREAM, BASE, RUST, BG = "#15110E", "#EEE4CF", "#1B1612", "#B4532A", "#0A0908"
PH = bird.phases(8); GLIDE = bird.GLIDE
CW, CH, PITCH = 360, 270, 392          # cell (4:3 plate) and pitch
CX, CY = 960, 540
T_CUT = 3.6                            # beat 7 at 100 bpm: the hit
BS = 0.42                              # bird scale inside a cell
ZOOM = 5.9                             # push into the centre plate

def cell(k):
    x = CX + k*PITCH
    L = [{"shape": "rect", "x": x, "y": CY, "width": CW, "height": CH,
          "fill": "radial-gradient(#F4ECDA, #D9CCB0)"}]
    # Muybridge-style measuring grid
    for gx in range(-5, 6):
        L.append({"shape": "rect", "x": x+gx*32, "y": CY, "width": 1.2 if gx else 1.6, "height": CH-12,
                  "fill": INK, "opacity": 0.16 if gx else 0.3})
    for gy in range(-4, 5):
        L.append({"shape": "rect", "x": x, "y": CY+gy*32, "width": CW-12, "height": 1.2 if gy else 1.6,
                  "fill": INK, "opacity": 0.16 if gy else 0.3})
    L.append({"text": f"PL. {k+7:02d}", "font": "JetBrains Mono", "size": 13, "color": INK,
              "opacity": 0.7, "x": x-CW/2+12, "y": CY+CH/2-12, "anchor": "bottomLeft"})
    L.append({"text": f"{(k+4)*0.042:.3f}s", "font": "JetBrains Mono", "size": 13, "color": INK,
              "opacity": 0.55, "x": x+CW/2-12, "y": CY+CH/2-12, "anchor": "bottomRight"})
    if k != 0:
        L.append({"shape": "path", "d": PH[(k+4) % 8], "fill": INK, "x": x, "y": CY-6, "scale": BS})
    else:
        # the centre plate comes alive: a flipbook at one phase per frame, then locks into the glide
        for j, d in enumerate(PH):
            L.append({"shape": "path", "d": d, "fill": INK, "x": x, "y": CY-6, "scale": BS,
                      "opacity": f"=gt < 1.75 ? ({j} == 4 ? 1 : 0) : (gt >= 3.3 ? 0 : (floor(gt*24) % 8 == {j} ? 1 : 0))"})
        L.append({"shape": "path", "d": GLIDE, "fill": INK, "x": x, "y": CY-6, "scale": BS,
                  "opacity": "=gt >= 3.3 ? 1 : 0"})
    return L

strip = []
# film base and backlit sprocket holes
strip.append({"shape": "rect", "x": CX, "y": CY, "width": 30*PITCH, "height": CH+150, "fill": BASE})
for s in range(-40, 41):
    for yy in (CY-CH/2-44, CY+CH/2+44):
        strip.append({"shape": "rect", "x": CX + s*PITCH/4 + PITCH/8, "y": yy, "width": 46, "height": 34,
                      "radius": 6, "fill": "#E9DCC0", "opacity": 0.9})
for k in range(-7, 8):
    strip += cell(k)
# edge code printed on the film base
for k in range(-7, 8, 2):
    strip.append({"text": f"KESTREL  ▸  {k+7:02d}A", "font": "JetBrains Mono", "size": 12, "color": "#C9A15E",
                  "opacity": 0.55, "x": CX + k*PITCH - 150, "y": CY+CH/2+14, "anchor": "topLeft", "letterSpacing": "0.2em"})

flicker = "=0.9 + 0.06*noise(gt*31) + 0.04*sin(gt*150)"
scene1 = {
    "duration": T_CUT, "background": BG,
    "camera": {"x": -1650, "zoom": 1.22, "animate": [
        {"to": {"x": 0}, "duration": 2.1, "ease": "outQuart"},
        {"to": {"zoom": ZOOM}, "at": 2.15, "duration": T_CUT-2.15, "ease": "inOutQuart"}],
        "shake": {"amount": 1.2, "frequency": 9}},
    "layers": [
        {"shape": "circle", "width": 2600, "height": 1500, "fill": "radial-gradient(#3A2A1C, #0A090800)", "opacity": 0.8},
        {"type": "group", "layers": strip, "opacity": flicker},
        {"shape": "rect", "x": CX, "y": CY, "width": 1920, "height": 1080, "fill": "#000",
         "opacity": [{"from": {"opacity": 1}, "to": {"opacity": 0}, "duration": 0.35}] and 1,
         "animate": [{"from": {"opacity": 1}, "to": {"opacity": 0}, "duration": 0.4, "ease": "outCubic"}]},
    ],
}
# scene 2: the negative. Bird inverted to cream on black, settles into the mark; wordmark.
BIG = BS*ZOOM
LS, LY = 0.50, 405       # logo bird scale and centre
scene2 = {
    "duration": 6.0 - T_CUT, "background": BG, "transition": "cut",
    "layers": [
        {"shape": "circle", "width": 2200, "height": 1300, "fill": "radial-gradient(#2A1F16, #0A090800)", "y": "44%",
         "opacity": flicker},
        # the plate the bird came from, redrawn as the mark's frame
        {"shape": "rect", "x": CX, "y": LY, "width": LS/BS*CW, "height": LS/BS*CH, "fill": "none",
         "stroke": {"color": CREAM, "width": 1.5}, "opacity": 0.45, "start": 0.35,
         "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "duration": 0.9, "ease": "inOutCubic"}]},
        {"shape": "rect", "x": CX, "y": LY+6*LS, "width": LS/BS*CW-24, "height": 1, "fill": CREAM, "start": 0.5,
         "animate": [{"from": {"opacity": 0, "scaleX": 0}, "to": {"opacity": 0.22, "scaleX": 1}, "duration": 0.8, "ease": "outExpo"}]},
        {"shape": "rect", "x": CX, "y": LY+6*LS, "width": 1, "height": LS/BS*CH-24, "fill": CREAM, "start": 0.5,
         "animate": [{"from": {"opacity": 0, "scaleY": 0}, "to": {"opacity": 0.22, "scaleY": 1}, "duration": 0.8, "ease": "outExpo"}]},
        {"text": "PL. 07", "font": "JetBrains Mono", "size": 15, "color": CREAM, "letterSpacing": "0.15em",
         "x": CX - LS/BS*CW/2 + 12, "y": LY + LS/BS*CH/2 - 12, "anchor": "bottomLeft", "start": 0.9,
         "animate": [{"from": {"opacity": 0}, "to": {"opacity": 0.55}, "duration": 0.4}]},
        {"text": "0.168s", "font": "JetBrains Mono", "size": 15, "color": CREAM, "letterSpacing": "0.15em",
         "x": CX + LS/BS*CW/2 - 12, "y": LY + LS/BS*CH/2 - 12, "anchor": "bottomRight", "start": 0.95,
         "animate": [{"from": {"opacity": 0}, "to": {"opacity": 0.55}, "duration": 0.4}]},
        {"shape": "path", "d": GLIDE, "fill": CREAM, "x": CX, "y": CY-6*ZOOM, "scale": BIG,
         "animate": [{"to": {"scale": LS, "y": LY}, "duration": 1.0, "ease": "outExpo"}]},
        {"text": "KESTREL", "font": "Instrument Serif", "size": 104, "color": CREAM, "letterSpacing": "0.42em",
         "x": CX + 22, "y": 712, "enter": "mask by line up", "start": 0.45},
        {"shape": "rect", "x": CX, "y": 792, "width": 120, "height": 3, "fill": RUST, "start": 0.7,
         "animate": [{"from": {"scaleX": 0}, "to": {"scaleX": 1}, "duration": 0.6, "ease": "outExpo"}]},
        {"shape": "rect", "x": CX, "y": CY, "width": 1920, "height": 1080, "fill": "#000", "opacity": 0,
         "animate": [{"to": {"opacity": 1}, "at": 2.1, "duration": 0.3, "ease": "inCubic"}]},
    ],
}
del scene1["layers"][2]["opacity"]

music = {
    "bpm": 100, "key": "D", "length": "6s", "seed": 7, "end": "ring",
    "master": {"lufs": -15, "ceiling": -1, "reverb": "hall"},
    "tracks": [
        {"inst": "drums", "drums": {"shaker": "XoxoXoxoXoxoXoxoXoxoXoxo........", "rim": "x...x...x...x...x...x...........", },
         "bars": 2, "gain": -9, "reverb": 0.05},
        {"inst": "strings", "notes": "o2 [D2 A2]/1 | [D2 A2]/2 [D2 A2 F#3 A3 E4]/1!", "gain": -3, "reverb": 0.5,
         "automate": {"volume": [[0, -16], [5.5, 0], [6, 2]]}},
        {"inst": "cello", "notes": "o2 D2/1 | D2/2 D2/1", "gain": -6, "reverb": 0.3},
        {"inst": "harp", "notes": "o3 r/1 | l16 D3 A3 D4 E4 F#4 A4 D5 E5 r/4 r/8 [F#5 A5]/4", "gain": -5, "reverb": 0.5},
        {"inst": "timpani", "notes": "o2 r/1 | r/4 (D2/16?)x4 D2/2!", "gain": -2, "reverb": 0.4},
        {"inst": "piano", "notes": "o1 r/1 | r/2 [D1 D2 A2]/2!", "gain": -4, "reverb": 0.5},
        {"inst": "glockenspiel", "notes": "o5 r/1 | r/2 r/4 r/8 [F#5 D6]/4", "gain": -10, "reverb": 0.6},
    ],
    "sfx": [{"sfx": "reverse-cymbal", "at": 3.6}, {"sfx": "boom", "at": 3.6}],
}
spec = {"size": "16:9", "fps": 24, "intent": {"pace": "steady"}, "background": BG,
        "post": {"grain": 0.05, "grainSize": 1.4, "vignette": 0.4, "halation": 0.25, "bloom": 0.15, "contrast": 1.05},
        "motionBlur": {"shutter": 180}, "music": music, "scenes": [scene1, scene2]}
json.dump(spec, open("spec.json", "w"), indent=1)
print("ok", len(json.dumps(spec)))
