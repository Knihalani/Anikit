"""9:16 cut: its own portrait sound report, 16 s at 120 bpm (8 bars), loops back to its first frame."""
from build import *
import build as B

def tall(_):
    f = Form(1080, 1920, [60, 470, 530, 590, 650, 710, 1020], 720, 770, 112, 8,
             {"title": (60, 150, 92), "sub": (64, 214), "PRODUCTION": (60, 330, 960), "RECORDER": (60, 430, 960),
              "DATE": (60, 630, 440), "ROLL": (560, 630, 460), "foot": 1860})
    f.heads = ["SUBJECT", "1", "2", "3", "4", "NOTES  ○ = KEEPER"]
    ink = Ink()
    cw = CW * 50 / 56
    ink.hand("field recording", 190, 300, -1, 0.01, size=50)
    old_end = ink.hand("the old one", 190, 400, -1, 0.01, size=50)
    ink.hand("1", 690, 600, -1, 0.01, size=50)
    ex = [("dawn chorus", "FS — missed it", -0.5), ("kick, live room", "NG — clipped", 2.0), ("rain on tin roof", "INC — batt", 2.5)]
    for i, (subj, note, at) in enumerate(ex):
        y = f.ry(i)
        ink.hand(subj, 72, y, -1, 0.01, size=50); ink.tick(500, y, -1)
        ink.hand(note, 722, y, at, 0.9 if i == 0 else 0.45, size=40)
    ink.stroke("M182 406 C 250 400, 320 410, %d 402" % (old_end + 8), 4.4, 0.3, 5)
    tern_x = old_end + 70
    tern_end = ink.hand("Tern", tern_x, 392, 6.0, 0.4, size=84, rot=-3)
    keep = [("blackbird, 5:12am", "got the top", 7.0, 7.8, 1), ("door, slammed", "no clip", 8.0, 8.7, 2),
            ("thunder, overhead", "still no clip", 9.2, 10.8, 3), ("the whole night", "30 hrs", 12.0, 12.6, 4)]
    for k, (subj, note, t0, tc, seed) in enumerate(keep):
        y = f.ry(3 + k)
        ink.hand(subj, 72, y, t0, 0.4, size=50)
        for j in range(4): ink.tick(500 + 60 * j, y, t0 + 0.3 + 0.06 * j)
        ink.hand(note, 722, y, t0 + 0.5, 0.35, size=40)
        w = CW * 40 / 56 * len(note)
        big = k == 2
        ink.circle(722 + w / 2, y, w / 2 + (40 if big else 22), 48 if big else 40, tc, 0.4, seed=seed, width=6 if big else 4.5)
    ink.circle((tern_x + tern_end) / 2, 392, (tern_end - tern_x) / 2 + 28, 46, 13.45, 0.45, seed=7, width=5)
    ink.hand("rolls the moment you lift it.", 190, 470, 6.45, 0.6, size=40)
    ink.hand("every take's a keeper.", 190, 528, 14.3, 0.7, size=44)
    W, H = f.w, f.h
    comp = hires(f, 16, STOCK["pink"], printed(f, "PINK — MIXER COPY") + ink.render())
    def c(cx, cy, z): return B.cam(cx, cy, z, W, H)
    def sc(start, dur, camera, extra=None):
        return {"duration": dur, "background": "#4A4D55", "camera": camera,
                "layers": [{"composition": "page", "width": W * SS, "height": H * SS, "scale": 1 / SS, "x": W / 2, "y": H / 2, "trimStart": start}] + (extra or []) + [light(W, H)]}
    y0, y6 = f.ry(0), f.ry(5)
    flash = {"shape": "rect", "width": W, "height": H, "x": W / 2, "y": H / 2, "fill": "#EEF2FF", "blend": "screen", "opacity": 0,
             "animate": [{"keyframes": {"opacity": [0, 0.45, 0.05, 0.3, 0]}, "at": 0.75, "duration": 0.3}]}
    first = c(820, y0 - 20, 2.3)
    scenes = [
        sc(0, 2, {**first, "animate": [move(first, c(840, y0, 2.15), 0, 2, "outCubic")]}),
        sc(2, 2, {**c(560, f.ry(1) - 40, 1.35)}),
        sc(4, 3.0, {**c(420, 400, 1.9), "animate": [move(c(420, 400, 1.9), c(440, 420, 1.75), 1.9, 0.4, "outCubic")]}),
        sc(7, 2.0, {**c(560, f.ry(4), 1.3)}),
        sc(9, 3.0, {**c(300, y6, 2.0), "shake": {"amount": 12, "at": 0.8, "duration": 0.5},
                    "animate": [move(c(300, y6, 2.0), c(560, y6, 1.15), 1.2, 1.0)]}, [flash]),
        sc(12, 1.2, {**c(540, 1000, 1.0)}),
        sc(13.2, 2.3, {**c(360, 450, 2.2), "animate": [move(c(360, 440, 2.2), c(370, 465, 1.95), 0.9, 1.2, "inOutCubic")]}),
        sc(15.5, 0.5, {**first}),
    ]
    music = {"style": "acoustic", "bpm": 120, "key": "D", "mood": "warm bright", "seed": 4, "end": "cut",
             "sections": [{"name": "intro", "bars": 3, "energy": 0.5}, {"name": "verse", "bars": 5, "energy": 0.85, "stop": 2}],
             "hits": ["@bar4"]}
    sound = {"palette": "organic", "density": 0.3, "ambience": {"type": "room", "level": -14},
             "cues": [{"sfx": "boom", "at": 9.8, "intensity": 1}, {"sfx": "rain", "at": 9.8, "duration": 2.2, "gain": -12},
                      {"sfx": "thump", "at": 8.0, "gain": -6}, {"sfx": "tape-stop", "at": 4.9, "gain": -10}]}
    return {"size": "9:16", "fps": 30, "compositions": {"page": comp}, "post": {"grain": 0.035, "vignette": 0.12},
            "music": music, "sound": sound, "scenes": scenes}
