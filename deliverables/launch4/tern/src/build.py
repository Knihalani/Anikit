"""Tern launch film: a sound report where the recordist stops writing excuses and starts circling.
python3 build.py wide  -> ../spec.json      (16:9 hero, 30 s, 96 bpm = 12 bars)
python3 build.py tall  -> ../vertical.json  (9:16, 15 s)"""
import json, sys
from circle import loop

INK, BIC, CARBON = "#2A2427", "#2343A6", "#4B5687"
STOCK = {"pink": "#F2C4CB", "yellow": "#F3E3A2", "white": "#F4F2EC"}
HAND, FORM = "Reenie Beanie", "Archivo"
CW = 17.5  # approx px per char of the hand at 56 px

class Form:
    def __init__(s, w, h, cols, head_y, row0, row_h, nrows, hdr):
        s.w, s.h, s.cols, s.head_y, s.row0, s.row_h, s.nrows, s.hdr = w, h, cols, head_y, row0, row_h, nrows, hdr
    def ry(s, i): return s.row0 + i * s.row_h + s.row_h / 2

WIDE = Form(1920, 1080, [80, 520, 610, 720, 830, 940, 1050, 1840], 344, 392, 76, 8,
            {"title": (80, 150, 86), "sub": (84, 212), "PRODUCTION": (1000, 130, 560), "DATE": (1620, 130, 220),
             "RECORDER": (1000, 214, 560), "ROLL": (1620, 214, 220), "foot": 1046})

def label(text, x, y, size=17, **kw):
    d = {"text": text, "font": FORM, "size": size, "weight": 600, "axes": {"wdth": 75}, "letterSpacing": "0.06em",
         "color": INK, "x": x, "y": y, "anchor": "left"}
    d.update(kw); return d

def printed(f, copy_label):
    L, X0, X1 = [], f.cols[0], f.cols[-1]
    L.append({"line": [X0, f.head_y - 26, X1, f.head_y - 26], "stroke": INK, "width": 3})
    L.append({"line": [X0, f.row0, X1, f.row0], "stroke": INK, "width": 2})
    for i in range(1, f.nrows + 1):
        y = f.row0 + i * f.row_h
        L.append({"line": [X0, y, X1, y], "stroke": INK, "width": 1.2, "opacity": 0.8})
    for x in f.cols[1:-1]:
        L.append({"line": [x, f.head_y - 26, x, f.row0 + f.nrows * f.row_h], "stroke": INK, "width": 1.2, "opacity": 0.8})
    lay = [{"draw": L, "inkTexture": 0.12}]
    tx, ty, ts = f.hdr["title"]
    lay.append(label("SOUND REPORT", tx, ty, size=ts, weight=800, axes={"wdth": 62}, letterSpacing="-0.01em"))
    lay.append(label("FIELD · STUDIO · SET", *f.hdr["sub"]))
    for k in ("PRODUCTION", "DATE", "RECORDER", "ROLL"):
        x, y, w = f.hdr[k]
        L.append({"line": [x + 118, y, x + w, y], "stroke": INK, "width": 1.2})
        lay.append(label(k, x, y - 9, anchor="bottomLeft"))
    heads = getattr(f, "heads", None) or ["SCENE / SUBJECT", "TAKE", "TR 1", "TR 2", "TR 3", "TR 4", "NOTES   ○ = CIRCLE TAKE   NG · FS · INC · WT · RT"]
    for c, h in zip(f.cols, heads):
        lay.append(label(h, c + (10 if getattr(f, "heads", None) else 16), f.head_y))
    lay.append(label(copy_label, X0, f.hdr["foot"], size=15))
    lay.append(label("CARBONLESS — PRESS FIRMLY", X1, f.hdr["foot"], size=15, anchor="right"))
    return lay

class Ink:
    """Collects hand-written layers; renders them as ballpoint (top sheet) or carbon (copies)."""
    def __init__(s): s.items = []
    def hand(s, text, x, y, at, dur=None, size=56, rot=-1.2):
        text = text.replace(" ", "\u2002")
        dur = dur if dur is not None else max(0.18, 0.045 * len(text))
        s.items.append({"text": text, "font": HAND, "size": size, "color": BIC, "x": x, "y": y + 4, "anchor": "left",
                        "rotation": rot, "start": at, "enter": {"preset": "wipe", "duration": dur, "ease": "inOutQuad"},
                        "inkTexture": {"amount": 0.3, "scale": 0.6, "seed": len(s.items)},
                        "animator": {"by": "char", "y": 3.5 * size / 56, "rotation": 4, "scale": 1.05, "start": 0, "end": 0.45,
                                     "order": "random", "seed": len(s.items) * 7 + 3, "smooth": 1}})
        return x + CW * len(text) * size / 56
    def stroke(s, d, at, dur, width=4.5):
        import re
        nums = [float(v) for v in re.findall(r"-?\d+\.?\d*", d)]
        xs, ys = nums[0::2], nums[1::2]
        s.items.append({"shape": "path", "d": d, "x": (min(xs) + max(xs)) / 2, "y": (min(ys) + max(ys)) / 2,
                        "stroke": BIC, "strokeWidth": width * 1.25, "fill": None,
                        "brush": {"taper": [0.15, 0.35], "pressure": 0.45, "seed": len(s.items)}, "start": at,
                        "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "duration": dur, "ease": "inOutSine"}]})
    def circle(s, cx, cy, rx, ry, at, dur=0.42, seed=1, width=4.5):
        s.stroke(loop(cx, cy, rx, ry, seed=seed), at, dur, width)
    def tick(s, x, y, at):
        import random
        r = random.Random(len(s.items) * 13 + int(x))
        j = lambda a: a + r.uniform(-3.5, 3.5)
        s.stroke("M%.1f %.1f Q%.1f %.1f %.1f %.1f Q%.1f %.1f %.1f %.1f" % (
            j(x - 15), j(y), j(x - 9), j(y + 8), j(x - 4), j(y + 13), j(x + 4), j(y - 2), j(x + 17), j(y - 17 - r.uniform(0, 6))),
            at, 0.12 + r.uniform(0, 0.05), 3.6 + r.uniform(0, 1.4))
    def render(s, carbon=False):
        out = []
        for it in s.items:
            it = json.loads(json.dumps(it))
            if carbon:
                it["opacity"] = 0.55; it["blur"] = 1.2
                it["x"] = it.get("x", 0) + 3; it["y"] = it.get("y", 0) + 2
                if "color" in it: it["color"] = CARBON
                if "stroke" in it: it["stroke"] = CARBON
            out.append(it)
        return out

PAPER = open(__file__.rsplit("/", 1)[0] + "/paper.glsl").read()
SS = 2  # pages are drawn at 2x so macro shots stay sharp

def hires(f, dur, bg, layers):
    return {"size": [f.w * SS, f.h * SS], "duration": dur, "background": bg,
            "layers": [{"type": "group", "width": f.w, "height": f.h, "x": f.w * SS / 2, "y": f.h * SS / 2, "scale": SS,
                        "layers": layers + [{"shader": PAPER, "blend": "multiply", "width": f.w, "height": f.h,
                                                      "x": f.w / 2, "y": f.h / 2}]}]}

def light(w, h, center=(0.28, 0.18)):
    return {"shape": "rect", "width": w, "height": h, "x": w / 2, "y": h / 2, "blend": "multiply",
            "fill": {"type": "radial", "center": list(center), "radius": 1.15,
                     "stops": [[0, "#ffffff"], [0.6, "#e9e1e4"], [1, "#c9bfc4"]]}}

def cam(cx, cy, z, W=1920, H=1080):
    if z >= 1:
        cx = min(max(cx, W / 2 / z), W - W / 2 / z); cy = min(max(cy, H / 2 / z), H - H / 2 / z)
    return {"zoom": z, "x": cx - W / 2, "y": cy - H / 2}

def move(a, b, at, dur, ease="inOutCubic"):
    return {"from": a, "to": b, "at": at, "duration": dur, "ease": ease}

# ---------------------------------------------------------------- the take log (shared story)
EXCUSES = [("dawn chorus", "1", "FS — missed the top"),
           ("kick, live room", "3", "NG — clipped"),
           ("rain on tin roof", "1", "INC — batt died"),
           ("the riff, 2am", "1", "FS — wasn't rolling")]
KEEPERS = [("blackbird, 5:12am", "1", "got the top"),
           ("door, slammed", "2", "no clip"),
           ("thunder, overhead", "1", "still no clip"),
           ("the whole night", "1", "30 hrs, one roll")]

def wide():
    f, ink = WIDE, Ink()
    T = {}  # key times (global = composition time)
    # header, already filled in when the film opens
    ink.hand("field recording", 1128, 104, -1, 0.01)
    ink.hand("1", 1750, 188, -1, 0.01)
    old_end = ink.hand("the old one", 1128, 188, -1, 0.01)
    # rows 0-3: excuses (subjects pre-written; the notes arrive on the beat)
    note_at = [-0.2, 2.5, 3.125, 3.75]
    for i, (subj, take, note) in enumerate(EXCUSES):
        y = f.ry(i)
        ink.hand(subj, 96, y, -1, 0.01); ink.hand(take, 552, y, -1, 0.01); ink.tick(665, y, -1)
        if i == 0:
            ink.hand("FS —", 1070, y, -1, 0.01)
            ink.hand("missed the top", 1070 + CW * 7.5, y, -0.45, 1.2)
        else:
            ink.hand(note, 1070, y, note_at[i], 0.5)
    # the recorder field: the old one is struck out, the new one written on the downbeat of bar 4
    ink.stroke("M1122 %d C 1180 %d, 1250 %d, %d %d" % (194, 186, 199, old_end + 40, 188), 5.6, 0.3, 5)
    tern_x = old_end + 80
    T["tern"] = 7.5
    tern_end = ink.hand("Tern", tern_x, 180, T["tern"], 0.45, size=84, rot=-3)
    # rows 4-7: keepers
    rows_at = [8.9, 11.25, 13.9, 19.0]
    circ = [(10.45, 1), (12.9, 2), (16.3, 3), (20.35, 4)]
    for k, (subj, take, note) in enumerate(KEEPERS):
        i, t0 = 4 + k, rows_at[k]
        y = f.ry(i)
        ink.hand(subj, 96, y, t0, 0.55); ink.hand(take, 552, y, t0 + 0.55, 0.1)
        for j in range(4): ink.tick(665 + 110 * j, y, t0 + 0.7 + 0.1 * j)
        nt = {0: 9.85, 1: 12.35, 2: 15.55, 3: 19.75}[k]
        ink.hand(note, 1070, y, nt, 0.5)
        w = CW * len(note)
        big = (k == 2)
        ink.circle(1070 + w / 2 - 4, y, w / 2 + (44 if big else 34), 46 if big else 35, circ[k][0],
                   0.5 if big else 0.42, seed=circ[k][1], width=6 if big else 4.5)
    # the ending: the name gets the last circle, and the margin note
    ink.circle((tern_x + tern_end) / 2, 180, (tern_end - tern_x) / 2 + 30, 44, 26.0, 0.5, seed=7, width=5)
    ink.hand("rolls the moment you lift it.", 1128, 254, 8.0, 0.75, size=46)
    ink.hand("every take's a keeper.", 1128, 302, 27.2, 0.75, size=46)

    def page(stock, carbon, lab):
        return hires(f, 30, STOCK[stock], printed(f, lab) + ink.render(carbon))
    comps = {"pink": page("pink", False, "PINK — MIXER COPY"),
             "yellow": page("yellow", True, "YELLOW — PRODUCTION"),
             "white": page("white", True, "WHITE — POST")}
    desk = "#4A4D55"

    def sheet(name, start, **kw):
        d = {"composition": name, "width": f.w * SS, "height": f.h * SS, "scale": 1 / SS, "x": f.w / 2, "y": f.h / 2, "trimStart": start}
        d.update(kw); return d

    def sc(start, dur, camera=None, extra=None, under=None):
        layers = (under or []) + [sheet("pink", start)] + (extra or []) + [light(f.w, f.h)]
        return {"duration": dur, "background": desk, "camera": camera, "layers": layers}

    y0, y4, y5, y6, y7 = f.ry(0), f.ry(4), f.ry(5), f.ry(6), f.ry(7)
    flash = {"shape": "rect", "width": 1920, "height": 1080, "x": 960, "y": 540, "fill": "#EEF2FF", "blend": "screen", "opacity": 0,
             "animate": [{"keyframes": {"opacity": [0, 0.45, 0.05, 0.3, 0]}, "at": 1.18, "duration": 0.3}]}
    nx = lambda note: 1070 + CW * len(note) / 2   # centre of a note
    scenes = [
        # 1 hook: macro on the excuse being written (0-2.5)
        sc(0, 2.5, {**cam(1290, y0 - 20, 2.6), "animate": [move(cam(1290, y0 - 20, 2.6), cam(1330, y0 - 10, 2.45), 0, 2.5, "outCubic")]}),
        # 2 the page of excuses, one per beat (2.5-5.0)
        sc(2.5, 2.5, {**cam(1100, 520, 1.6), "animate": [move(cam(1100, 520, 1.6), cam(1140, 530, 1.5), 0, 2.5, "outCubic")]}),
        # 3 the recorder field: the old one struck out (5.0-6.25)
        sc(5.0, 1.25, {**cam(1260, 200, 2.5)}),
        # 4 the stop: silence, an empty line waiting (6.25-7.5)
        sc(6.25, 1.25, {**cam(1400, 196, 3.6)}),
        # 5 the downbeat: the new name, and what it does (7.5-8.75)
        sc(7.5, 1.25, {**cam(1330, 215, 2.25)}),
        # 6 blackbird row (8.75-10.25)
        sc(8.75, 1.5, {**cam(700, y4 - 20, 1.6), "animate": [move(cam(700, y4 - 20, 1.6), cam(900, y4 - 20, 1.6), 0.6, 0.7)]}),
        # 7 cut-in: the first circle, macro (10.25-11.25)
        sc(10.25, 1.0, {**cam(nx("got the top"), y4, 3.4)}),
        # 8 door slam: the page jolts, no clip, circled (11.25-13.75)
        sc(11.25, 2.5, {**cam(1050, y5 - 40, 1.55), "shake": {"amount": 8, "at": 0.62, "duration": 0.35},
                         "animate": [move(cam(1050, y5 - 40, 1.55), cam(1180, y5 - 20, 1.75), 0.95, 0.7)]}),
        # 9 thunder: the magic shot, one take: write, the hit, the circle, held (13.75-18.75)
        sc(13.75, 5.0, extra=[flash],
           camera={**cam(420, y6 - 30, 1.9), "shake": {"amount": 14, "at": 1.25, "duration": 0.6},
                   "animate": [move(cam(420, y6 - 30, 1.9), cam(770, y6 - 60, 1.3), 1.55, 1.3)]}),
        # 10 thirty hours (18.75-21.25)
        sc(18.75, 2.5, {**cam(700, y7 - 60, 1.7), "animate": [move(cam(700, y7 - 60, 1.7), cam(1150, y7 - 40, 1.8), 0.85, 0.9)]}),
        # 11 the whole report, and its copies beneath (21.25-25)
        sc(21.25, 3.75, {**cam(960, 540, 1.0), "animate": [move(cam(960, 540, 1.0), cam(1010, 600, 0.84), 0.2, 2.4, "inOutCubic")]},
           under=[sheet("white", 21.25, x=f.w / 2 + 70, y=f.h / 2 + 64, rotation=1.4,
                        shadow={"color": "#00000055", "blur": 18, "y": 6}),
                  sheet("yellow", 21.25, x=f.w / 2 + 36, y=f.h / 2 + 30, rotation=0.6,
                        shadow={"color": "#00000044", "blur": 14, "y": 4})]),
        # 12 the name gets the last circle (25-27)
        sc(25.0, 2.0, {**cam(1460, 185, 3.3), "animate": [move(cam(1460, 185, 3.3), cam(1460, 190, 3.1), 0, 2.0, "outCubic")]}),
        # 13 the promise, in the margin (27-30)
        sc(27.0, 3.0, {**cam(1330, 230, 2.05), "animate": [move(cam(1330, 230, 2.05), cam(1340, 235, 2.25), 0, 3.0, "inOutSine")]}),
    ]
    music = {"style": "acoustic", "bpm": 96, "key": "D", "mood": "warm bright", "seed": 4, "end": "ring",
             "sections": [{"name": "intro", "bars": 3, "energy": 0.25},
                          {"name": "verse", "bars": 6, "energy": 0.55, "stop": 2},
                          {"name": "outro", "bars": 3, "energy": 0.4}],
             "hits": ["@bar4", "@bar7"],
             "tracks": [{"inst": "flute", "notes": "@4 r2 o6 l16 A B A F# A/8 r8 D7/16 B/16 A/8", "gain": -6, "reverb": 0.5}]}
    sound = {"palette": "organic", "density": 0.3, "ambience": {"type": "room", "level": -24},
             "cues": [{"sfx": "page", "at": 0.1, "gain": -12},
                      {"sfx": "thump", "at": 11.87, "gain": -2, "intensity": 0.8},
                      {"sfx": "boom", "at": 15.0, "gain": 0, "intensity": 1, "size": 1},
                      {"sfx": "rain", "at": 15.0, "duration": 3.5, "gain": -12},
                      {"sfx": "paper", "at": 21.4, "gain": -10},
                      {"sfx": "rain", "at": 3.0, "duration": 0.9, "gain": -18},
                      {"sfx": "night", "at": 18.9, "duration": 2.3, "gain": -14}]}
    return {"size": "16:9", "fps": 30, "intent": {"pace": "medium"}, "compositions": comps,
            "post": {"grain": 0.035, "vignette": 0.15}, "music": music, "sound": sound, "scenes": scenes}

if __name__ == "__main__":
    mode = sys.argv[1] if len(sys.argv) > 1 else "wide"
    spec = wide() if mode == "wide" else __import__("tall").tall(sys.modules[__name__])
    out = sys.argv[2] if len(sys.argv) > 2 else ("../spec.json" if mode == "wide" else "../vertical.json")
    json.dump(spec, open(out, "w"), indent=1)
    print("wrote", out)
