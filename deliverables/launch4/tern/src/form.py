"""The Tern sound report: a three-part carbonless form, drawn as a composition."""
from circle import loop

INK = "#2A2427"        # printed form ink
BIC = "#2343A6"        # ballpoint blue
STOCK = {"pink": "#F2C4CB", "yellow": "#F3E3A2", "white": "#F4F2EC"}
HAND = "Reenie Beanie"
FORM = "Archivo"

# table geometry (composition pixels, 1920x1080)
X0, X1 = 80, 1840
COLS = [80, 520, 610, 720, 830, 940, 1050, 1840]   # subject | take | tr1..tr4 | notes
HEAD_Y = 268
ROW0 = 318
ROW_H = 86
NROWS = 8

def label(text, x, y, size=17, **kw):
    d = {"text": text, "font": FORM, "size": size, "weight": 600, "axes": {"wdth": 75},
         "letterSpacing": "0.06em", "color": INK, "x": x, "y": y, "anchor": "left"}
    d.update(kw)
    return d

def printed(copy="pink", copy_label="PINK — MIXER COPY"):
    lines = []
    # outer table rules
    lines.append({"line": [X0, HEAD_Y - 26, X1, HEAD_Y - 26], "stroke": INK, "width": 3})
    lines.append({"line": [X0, ROW0, X1, ROW0], "stroke": INK, "width": 2})
    for i in range(1, NROWS + 1):
        y = ROW0 + i * ROW_H
        lines.append({"line": [X0, y, X1, y], "stroke": INK, "width": 1.2, "opacity": 0.8})
    for x in COLS[1:-1]:
        lines.append({"line": [x, HEAD_Y - 26, x, ROW0 + NROWS * ROW_H], "stroke": INK, "width": 1.2, "opacity": 0.8})
    # header field rules
    for (x, y, w) in [(1000, 92, 360), (1460, 92, 380), (1000, 168, 360), (1460, 168, 380)]:
        lines.append({"line": [x, y, x + w, y], "stroke": INK, "width": 1.2})
    layers = [
        {"draw": lines, "inkTexture": 0.15},
        label("SOUND REPORT", 80, 112, size=86, weight=800, axes={"wdth": 62}, letterSpacing="-0.01em"),
        label("FIELD · STUDIO · SET", 84, 176, size=17),
        label("PRODUCTION", 1000, 74), label("DATE", 1460, 74),
        label("RECORDER", 1000, 150), label("ROLL", 1460, 150),
        label("SCENE / SUBJECT", 96, HEAD_Y), label("TAKE", 536, HEAD_Y),
        label("TR 1", 626, HEAD_Y), label("TR 2", 736, HEAD_Y), label("TR 3", 846, HEAD_Y), label("TR 4", 956, HEAD_Y),
        label("NOTES  (○ = CIRCLE TAKE   NG · FS · INC · WT · RT)", 1066, HEAD_Y),
        label(copy_label, 80, 1046, size=15),
        label("CARBONLESS — PRESS FIRMLY", 1840, 1046, size=15, anchor="right"),
    ]
    return layers

def row_y(i):
    return ROW0 + i * ROW_H + ROW_H / 2

def hand(text, x, y, at, dur=0.6, size=56, rot=-1.5, **kw):
    d = {"text": text, "font": HAND, "size": size, "color": BIC, "x": x, "y": y + 4, "anchor": "left",
         "rotation": rot, "start": at, "enter": {"preset": "wipe", "duration": dur, "ease": "linear"}}
    d.update(kw)
    return d

def circle(cx, cy, rx, ry, at, dur=0.45, seed=1, width=5, **kw):
    d = {"draw": [{"d": loop(cx, cy, rx, ry, seed=seed), "stroke": BIC, "width": width, "cap": "round"}],
         "start": at, "opacity": 0.92,
         "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "duration": dur, "ease": "inOutSine"}]}
    d.update(kw)
    return d
