#!/usr/bin/env python3
"""Composes the launch film spec from one cue table (the firing script).

Every shell in the sky, every row of the script on screen and every boom in the
mix come from the same cue rows, so picture, type and sound can't drift apart.
The shells themselves are drawn inside the spec by src/shells.rhai (embedded as a
script layer), so spec.json is self-contained.

usage: film.py 16:9|9:16 out.json
"""
import json, sys, pathlib

HERE = pathlib.Path(__file__).parent
RHAI = (HERE / "shells.rhai").read_text().splitlines()
FMT = sys.argv[1] if len(sys.argv) > 1 else "16:9"
OUT = sys.argv[2] if len(sys.argv) > 2 else "spec.json"
V = FMT == "9:16"
W, H = (1080, 1920) if V else (1920, 1080)

NAVY, VERM, GOLD, COB = "#14213D", "#E8432A", "#E9A23B", "#2346B8"
DAY = {"type": "linear", "angle": 180, "stops": ["#CBDCE6", "#E4ECEE"]}
DUSK = {"type": "linear", "angle": 180, "stops": ["#7A92AA", "#BAC6CD"]}
MONO, DISP = "IBM Plex Mono", "Archivo"
M = 0.07 * W  # outer margin

NAMES = {"pistil": "PISTIL PEONY", "peony": "PEONY", "chrys": "CHRYSANTHEMUM", "ring": "RING",
         "willow": "GOLD WILLOW", "name": "FINALE"}
INKS = {VERM: "VERMILION", GOLD: "GOLD", COB: "COBALT"}

LETTERS = {
    "A": [".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"],
    "N": ["#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#"],
    "I": ["###", ".#.", ".#.", ".#.", ".#.", ".#.", "###"],
    "K": ["#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#"],
    "T": ["#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#.."],
}
def word_rows(word):
    rows = [""] * 7
    for n, ch in enumerate(word):
        for r in range(7):
            rows[r] += ("." if n else "") + LETTERS[ch][r]
    return rows

INKN = {VERM: "vermilion", GOLD: "gold", COB: "cobalt"}
def obj(c):
    """the readable cue object, exactly as it appears in the spec's script"""
    r3 = lambda v: round(float(v), 3)
    return {"at": r3(c[0]), "shell": c[4], "ink": INKN[c[5]], "x": r3(c[1]), "y": r3(c[2]), "r": r3(c[3]), "lift": r3(c[6])}
def rh(c):
    if c[4] == "name":
        return json.dumps(c)
    return "#" + json.dumps(obj(c), separators=(", ", ": "))
def shells(cues, mv=(0.0, 0.0), origins=()):
    og = "let origins = [" + ", ".join(json.dumps([round(a, 4), round(b, 4)]) for a, b in origins) + "];"
    return {"script": ["let cues = [", *[rh(c) + "," for c in cues], "];", f"let mv = [{float(mv[0])}, {float(mv[1])}];", og] + RHAI}

def calibre(R):
    return max(2, min(8, round(R * 22)))

def row_text(c, idx=None):
    return json.dumps(obj(c), separators=(", ", ": "))

def old_row_text(c, idx):
    t, kind, ink = c[0], c[4], c[5]
    mm, ss = divmod(t, 60)
    eff = NAMES[kind] if kind == "name" else f'{calibre(c[3])}" {NAMES[kind]}'
    return f"{int(mm):02d}:{ss:05.2f}  {idx}  {eff}"

def json_row(c):
    return json.dumps(c, separators=(", ", ": "))

def T(text, x, y, size, color=NAVY, font=MONO, **kw):
    d = {"text": text, "font": font, "size": size, "x": x, "y": y, "anchor": "left", "color": color}
    if font == MONO:
        d["typeset"] = False
        d["maxWidth"] = W
    if font == DISP:
        d["axes"] = {"wdth": 75, "wght": 800}
        d["letterSpacing"] = -0.01
    d.update(kw)
    return d

SFX = []
def sound_for(cues, offset=0.0, scale=1.0, xmap=lambda x: x, every=1):
    """booms on breaks, mortar thumps on lifts (picture-derived, same rows)."""
    for n, c in enumerate(cues):
        if n % every:
            continue
        t, x, R, kind = c[0] + offset, xmap(c[1]), c[3], c[4]
        big = R > 0.2 or kind in ("name",)
        SFX.append({"sfx": "boom" if big else "thump", "at": round(t, 3), "x": round(x, 3),
                    "gain": round((-4 if big else -9) * scale, 1), "size": 0.8 if big else 0.4})
        if kind == "willow":
            SFX.append({"sfx": "sparkle", "at": round(t + 0.15, 3), "x": round(x, 3), "gain": -12})
        if len(c) > 6 and c[6] > 0 and n % 2 == 0:
            SFX.append({"sfx": "thump", "at": round(t - c[6], 3), "x": round(x, 3), "gain": -16, "pitch": -5})

# ---------------------------------------------------------------- cue table
# [t (scene-local s), x, y, R, kind, ink, lift, xoff, ink2]
scenes = []

# S1 · bars 1-2 · first cue. Frame 0: a lift trail already rising; the bottom strip is the script.
s1 = [[0.0, 0.62, 0.53, 0.23, "pistil", VERM, 0.0, 0, GOLD],
      [1.0, 0.86, 0.50, 0.12, "peony", COB, 0.9],
      [2.00, 0.34, 0.30, 0.17, "ring", COB, 0.6],
      [3.00, 0.82, 0.26, 0.19, "chrys", GOLD, 0.6]]
if V:
    s1 = [[0.0, 0.50, 0.46, 0.16, "pistil", VERM, 0.0, 0, GOLD],
          [1.0, 0.80, 0.56, 0.07, "peony", COB, 0.9],
          [2.00, 0.28, 0.38, 0.09, "ring", COB, 0.6],
          [3.00, 0.70, 0.34, 0.10, "chrys", GOLD, 0.6]]
rows1 = []
ry = H * (0.86 if not V else 0.76)
for n, c in enumerate(s1):
    t0 = 0 if n == 0 else c[0] - 0.35
    t1 = s1[n + 1][0] - 0.35 if n + 1 < len(s1) else 4.0
    big = 28 if not V else 15
    rows1 += [
        {"shape": "rect", "width": W - 2 * M + 40, "height": big * 1.7, "fill": NAVY, "x": M - 20, "y": ry,
         "anchor": "left", "start": t0, "end": t1},
        T(row_text(c, f"A{n+1:02d}"), M, ry, big, color="#E4ECEE", start=t0, end=t1),
    ]
claim1 = T("Your agent writes the page." if not V else "Your agent\nwrites the page.", M, H * (0.14 if not V else 0.16), 112 if not V else 104, font=DISP, lineHeight=1.0)
o1 = [((M + 0.6 * big * len(row_text(c))) / W, ry / H) for c in s1]
scenes.append({"duration": 4.0, "background": DAY,
               "layers": rows1 + [shells(s1, origins=o1), claim1]})
sound_for(s1, 0.0)

# S2 · bars 3-4 · the script runs: column of rows, each fires on its beat
s2 = []
kinds = ["peony", "chrys", "pistil", "ring", "chrys", "peony", "pistil", "ring"]
inks = [COB, GOLD, VERM, GOLD, COB, VERM, COB, VERM]
pos16 = [(0.62, 0.30), (0.80, 0.22), (0.50, 0.24), (0.88, 0.36), (0.70, 0.36), (0.56, 0.34), (0.84, 0.28), (0.66, 0.20)]
pos9 = [(0.30, 0.50), (0.70, 0.46), (0.50, 0.38), (0.25, 0.62), (0.72, 0.62), (0.45, 0.55), (0.65, 0.40), (0.35, 0.42)]
for n in range(8):
    x, y = (pos9 if V else pos16)[n]
    R = (0.10 if V else 0.11) + 0.025 * (n % 3)
    c = [0.30 + 0.45 * n, x, y, R, kinds[n], inks[n], 0.55]
    if kinds[n] == "pistil":
        c += [0, GOLD if inks[n] != GOLD else VERM]
    s2.append(c)
L2 = []
lh = 40 if not V else 40
fs = 24 if not V else 22
top = H * 0.56 if not V else H * 0.08
for n, c in enumerate(s2):
    y = top + n * lh
    L2.append(T(row_text(c), M, y, fs))
    L2.append({"shape": "rect", "width": W - 2 * M + 24, "height": lh - 6, "fill": NAVY,
               "x": M - 12, "y": y, "anchor": "left", "start": c[0] - 0.02, "end": c[0] + 0.43})
    L2.append(T(row_text(c, f"B{n+1:02d}"), M, y, fs, color="#E4ECEE", start=c[0] - 0.02, end=c[0] + 0.43))
claim2 = T("Back comes a finished film,\nwith sound.", M, H * 0.14 if not V else H * 0.86, 84 if not V else 80,
           font=DISP, start=0.9, lineHeight=1.0, enter={"preset": "mask by line up", "duration": 0.5})
o2 = [((M + 0.6 * fs * len(row_text(c))) / W, (top + n * lh) / H) for n, c in enumerate(s2)]
scenes.append({"duration": 4.0, "background": DAY, "layers": L2 + [shells(s2, origins=o2), claim2]})
sound_for(s2, 4.0)

# S3 · bars 5-6 · the chain: the whole sky; the current row ticks at the top
s3 = []
import random
rnd = random.Random(7)
kc = ["chrys", "pistil", "peony", "willow", "ring"]
for n in range(22):
    t = 0.15 + n * 0.17
    k = kc[n % 5]
    ink = [VERM, GOLD, COB][n % 3]
    x = rnd.uniform(0.12, 0.88)
    y = rnd.uniform(0.22, 0.6) if not V else rnd.uniform(0.25, 0.65)
    R = rnd.uniform(0.12, 0.22) if not V else rnd.uniform(0.07, 0.13)
    c = [round(t, 2), round(x, 3), round(y, 3), round(R, 3), k, ink, 0.5]
    if k == "pistil":
        c += [0, VERM if ink != VERM else GOLD]
    s3.append(c)
L3 = []
for n, c in enumerate(s3):
    t1 = s3[n + 1][0] if n + 1 < len(s3) else 4.0
    L3.append(T(row_text(c), M, H * 0.08 if not V else H * 0.12, 26 if not V else 17, start=c[0], end=t1))
scenes.append({"duration": 4.0, "background": DAY, "layers": [shells(s3)] + L3})
sound_for(s3, 8.0, every=2)

# S4 · bars 7-9 · two nights, one page: split, then the halves slide into one
seg = []
segk = ["pistil", "chrys", "ring", "peony", "peony", "chrys", "pistil", "ring", "peony"]
segi = [VERM, COB, GOLD, GOLD, VERM, GOLD, COB, VERM, COB]
segp = [(0.25, 0.40), (0.14, 0.28), (0.36, 0.30), (0.22, 0.30), (0.33, 0.55), (0.15, 0.50), (0.27, 0.26), (0.38, 0.44), (0.20, 0.42)]
for n in range(9):
    x, y = segp[n]
    if V:
        y = 0.3 + y * 0.5
    R = 0.09 + 0.02 * (n % 3) if not V else 0.045 + 0.01 * (n % 3)
    c = [round(0.15 + 0.6 * n, 2), round(x, 3), round(y, 3), round(R, 3), segk[n], segi[n], 0.5]
    if segk[n] == "pistil":
        c += [0, GOLD if segi[n] != GOLD else VERM]
    seg.append(c)
MERGE = 3.9
if True:
    half = lambda dx, lab: shells([[c[0], c[1] + dx] + c[2:] for c in seg], (MERGE, 0.25 if dx == 0 else -0.25))
    lab = lambda txt, x: T(txt, x, H * (0.92 if not V else 0.78), 30 if not V else 22, end=MERGE + 0.1, exit={"preset": "fadeOut", "duration": 0.2})
    L4 = [half(0.0, "TUE"), half(0.5, "SAT"),
          {"shape": "rect", "width": 3, "height": H, "fill": NAVY, "x": W / 2, "y": H / 2, "end": MERGE + 0.3,
           "animate": [{"to": {"scaleY": 0}, "at": MERGE, "duration": 0.3, "ease": "inQuad"}]},
          lab("TUESDAY 21:00:00", M), lab("SATURDAY 21:00:00", W / 2 + M * 0.5),
          T("Same page. Same film.\nByte for byte." if not V else "Same page.\nSame film.\nByte for byte.", M, H * (0.14 if not V else 0.12), 84, font=DISP, start=MERGE + 0.7, lineHeight=1.0,
            enter={"preset": "mask by line up", "duration": 0.5}),
          T("TUESDAY = SATURDAY", M, H * (0.92 if not V else 0.78), 30 if not V else 22, start=MERGE + 0.4, enter={"preset": "fadeIn", "duration": 0.2})]
    sound_for(seg, 12.0, xmap=lambda x: x * 2 * 0.5 + 0.25)
else:
    half = lambda dy: {"children": [shells([[c[0], c[1], c[2] + dy] + c[3:] for c in seg])], "x": 0, "y": 0,
                       "width": W, "height": H, "anchor": "topLeft",
                       "animate": [{"to": {"y": (0.25 if dy == 0 else -0.25) * H}, "at": MERGE, "duration": 0.6, "ease": "inOutCubic"}]}
    lab = lambda txt, y: T(txt, M, y, 30, end=MERGE + 0.1, exit={"preset": "fadeOut", "duration": 0.2})
    L4 = [half(0.0), half(0.5),
          {"shape": "rect", "width": W, "height": 3, "fill": NAVY, "x": W / 2, "y": H / 2, "end": MERGE + 0.3,
           "animate": [{"to": {"scaleX": 0}, "at": MERGE, "duration": 0.3, "ease": "inQuad"}]},
          lab("TUESDAY 21:00:00", H * 0.12), lab("SATURDAY 21:00:00", H * 0.56),
          T("Same page.\nSame film.\nByte for byte.", M, H * 0.70, 84, font=DISP, start=MERGE + 0.7, lineHeight=1.0,
            enter={"preset": "mask by line up", "duration": 0.5}),
          T("TUESDAY = SATURDAY", M, H * 0.12, 30, start=MERGE + 0.4, enter={"preset": "fadeIn", "duration": 0.2})]
    sound_for(seg, 12.0)
scenes.append({"duration": 6.0, "background": DAY, "layers": L4})

# S5 · bar 10 · the hold: music stops, the sky dims, cursor on FINALE
blink = "=step(0.5, fract(t*2))"
L5 = [{"shape": "rect", "width": W, "height": H, "x": W / 2, "y": H / 2, "fill": DUSK, "opacity": 0,
       "animate": [{"to": {"opacity": 1}, "duration": 1.6, "ease": "inOutCubic"}]},
      T("@HOLD@", M, H * 0.5, 30 if not V else 17),
      {"shape": "rect", "width": 18 if not V else 10, "height": 40 if not V else 24, "fill": VERM,
       "x": "@HOLDX@", "y": H * 0.5, "opacity": blink}]
scenes.append({"duration": 2.0, "background": DAY, "layers": L5})

# S6 · bars 11-13 · the finale chain, then the name
fin = []
rnd = random.Random(11)
for n in range(18 if not V else 12):
    t = 0.05 + n * (0.15 if not V else 0.1)
    k = ["chrys", "pistil", "ring", "willow", "peony"][n % 5]
    ink = [GOLD, VERM, COB][n % 3]
    x = 0.1 + 0.8 * ((n * 0.618) % 1.0)
    y = rnd.uniform(0.2, 0.6)
    R = rnd.uniform(0.12, 0.22) if not V else rnd.uniform(0.08, 0.14)
    c = [round(t, 2), round(x, 3), round(y, 3), round(R, 3), k, ink, 0.45]
    if k == "pistil":
        c += [0, GOLD if ink != GOLD else VERM]
    fin.append(c)
NAME_T = 4.0 if not V else 2.4
pitch = 0.030 if not V else 0.0135
ncx, ncy = (0.5, 0.42) if not V else (0.5, 0.36)
fin.append([NAME_T - 0.02, ncx, ncy + 0.05, 0.2 if not V else 0.12, "pistil", GOLD, 0.7, 0, VERM])
fin.append([NAME_T, ncx, ncy, pitch, "name", VERM, 0, 0, word_rows("ANIKIT"), ncx, ncy + 0.05])
L6 = []
for n, c in enumerate(fin[:-1]):
    t1 = fin[n + 1][0] if n + 1 < len(fin) - 1 else NAME_T + 0.6
    L6.append(T(row_text(c), M, H * 0.08 if not V else H * 0.12, 26 if not V else 17, start=c[0], end=t1))
hold = row_text(fin[0])
L5[1]["text"] = hold
L5[2]["x"] = M + (30 if not V else 17) * 0.6 * len(hold) + 16
scenes.append({"duration": 6.0, "background": DUSK, "layers": [shells(fin)] + L6})
sound_for(fin, 20.0, every=2)

# S7 · bars 14-16 · the page shows its true form: these rows are the spec. Name + command + URL.
# carry the name (held as a static dot-matrix) and add the reveal
nm = word_rows("ANIKIT")
pp = pitch * H
nx0 = ncx * W - (len(nm[0]) - 1) * pp / 2
ny0 = ncy * H - 6 * pp / 2
dots = [{"circle": [round(nx0 + q * pp, 1), round(ny0 + r * pp, 1), round(pp * 0.33, 1)], "fill": VERM}
        for r in range(7) for q in range(len(nm[0])) if nm[r][q] == "#"]
jy = H * 0.66 if not V else H * 0.56
fsj = 26 if not V else 15
L7 = [{"draw": dots}]
# the end stays alive: the last rows of the page, each firing as the cursor reaches it
spots = [(0.86, 0.17), (0.72, 0.12), (0.93, 0.33), (0.79, 0.29)] if not V else [(0.80, 0.12), (0.25, 0.10), (0.62, 0.18), (0.40, 0.17)]
kinds7 = ["ring", "pistil", "peony", "chrys"]
inks7 = [GOLD, VERM, COB, GOLD]
last = [[round((2.4 + 0.8 * n if not V else 1.2 + 0.6 * n) + 0.25, 2), spots[n][0], spots[n][1], 0.07 if not V else 0.05, kinds7[n], inks7[n], 0.25] for n in range(4)]
for n, c in enumerate(last):
    y = jy + n * (fsj * 1.6)
    tt = c[0] - 0.25
    L7.append(T(row_text(c), M, y, fsj, start=0.9 + 0.12 * n, enter={"preset": "typewriter", "duration": 0.35}))
    L7.append({"shape": "rect", "width": W - 2 * M + 24, "height": fsj * 1.45, "fill": NAVY, "x": M - 12, "y": y,
               "anchor": "left", "start": tt, "end": tt + (0.78 if not V else 0.58)})
    L7.append(T(row_text(c), M, y, fsj, color="#E4ECEE", start=tt, end=tt + (0.78 if not V else 0.58)))
L7 += [T("One JSON spec in. A finished film out.", M, jy - fsj * 3.2, 56 if not V else 60, font=DISP,
         start=0.6, enter={"preset": "mask by line up", "duration": 0.5}, maxWidth=W - 2 * M),
       T("This film is one: every frame and note came from spec.json.", M, jy - fsj * 1.4, fsj, start=1.2,
         enter={"preset": "fadeIn", "duration": 0.3}),
       T("Anikit Video", M, H * (0.88 if not V else 0.71), 84 if not V else 92, font=DISP, start=0.3,
         enter={"preset": "mask by line up", "duration": 0.5}),
       T("github.com/knihalani/anikit", M + (0 if V else 0.42 * W), H * (0.88 if not V else 0.76), 34 if not V else 34,
         start=0.8, enter={"preset": "fadeIn", "duration": 0.3}),
       ]
L7.insert(1, shells(last))
scenes.append({"duration": 6.0, "background": DUSK, "layers": L7})
sound_for(last, 26.0)

# 9:16 cut: 20 s (10 bars) — first cue, the chain, the seam, the hold, the finale, the name. Loops to the empty sky.
if V:
    keep = [0, 2, 3, 4, 5, 6]
    durs = [4.0, 4.0, 6.0, 2.0, 4.0, 4.0]
    scenes = [dict(scenes[i], duration=durs[n]) for n, i in enumerate(keep)]

music = {"style": "orchestral", "bpm": 120, "key": "D", "mood": "bright triumphant",
         "sections": ([{"name": "intro", "bars": 2, "energy": 0.45}, {"name": "build", "bars": 4, "energy": 0.7},
                       {"name": "peak", "bars": 4, "energy": 0.9}, {"name": "finale", "bars": 3, "energy": 1, "stop": 4},
                       {"name": "outro", "bars": 3, "energy": 0.45}] if not V else
                      [{"name": "intro", "bars": 2, "energy": 0.55}, {"name": "build", "bars": 2, "energy": 0.75},
                       {"name": "peak", "bars": 4, "energy": 0.9}, {"name": "finale", "bars": 2, "energy": 1, "stop": 4},
                       {"name": "outro", "bars": 2, "energy": 0.45}]),
         "hits": ["@bar11", "@bar13"] if not V else ["@bar9", "@bar10"], "end": "ring", "followCuts": False}

if V:  # re-time sound cues to the vertical edit
    SFX.clear()
    sound_for(s1, 0.0)
    sound_for(s3, 4.0, every=2)
    sound_for(seg, 8.0)
    sound_for(fin, 16.0, every=2)
    sound_for(last, 20.0)

spec = {"size": FMT, "fps": 30,
        "intent": {"pace": "fast", "genre": "launch"},
        "fonts": {}, "music": music,
        "sound": {"design": "off", "space": "outdoor", "cues": SFX},
        "post": {"grain": 0.035},
        "scenes": scenes}
json.dump(spec, open(OUT, "w"), indent=1)
print(OUT, sum(1 for _ in SFX), "sfx")
