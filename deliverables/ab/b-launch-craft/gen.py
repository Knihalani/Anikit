#!/usr/bin/env python3
"""Railyard — "Arrivals". Generates spec.json: a split-flap departure board for money.

Every cell is a real split-flap: it can only advance through its flap alphabet in order,
one flap every FLIP seconds, so cells that travel further stop later (the natural stagger
of a Solari board). Each flip gets a falling-flap shadow; each burst gets a clatter in sound.
"""
import json, random, os

random.seed(7)
W, H = 1920, 1080
COLS, ROWS = 38, 7
PX, CW = 46, 42           # cell pitch, cell width
PY, CH = 100, 82          # row pitch, cell height
X0 = (W - (COLS * PX - (PX - CW))) // 2
Y0 = 236
FLIP = 0.034              # seconds per flap
MAXSTEPS = 22

ALPHA = " ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789$.,:/>+-"

ENAMEL = "#0B0F0D"
FLAP_T, FLAP_B = "#252826", "#1C1E1D"
INK = "#EFE8D8"
AMBER = "#F2A93B"
GREEN = "#3DDC84"
LABEL = "#8C8a80"


def cell_xy(r, c):
    return X0 + c * PX, Y0 + r * PY


def fields(sent, transfer, arrives, status):
    return f"{sent:<8} {transfer:<12} {arrives:<8} {status:<7}"


BLANK = " " * COLS

# ---- board states ---------------------------------------------------------
OLD = [
    fields("09:12:44", "WIRE  $8,200", "FRI", "DELAYED"),
    fields("09:20:05", "ACH   $2,315", "+2 DAYS", "QUEUED"),
    fields("09:33:51", "PAYROLL $41K", "THU", "HELD"),
    fields("09:38:10", "INV   $960", "MON", "QUEUED"),
    BLANK, BLANK, BLANK,
]
CALL = "POST /V1/TRANSFERS".ljust(COLS)
NEW = lambda st: fields("09:41:07", "$12,400.00", "09:41:07", st)
HOOK = "WEBHOOK > TRANSFER."


def hook_row(ev):
    return (" " * 9 + HOOK + ev).ljust(COLS)


PAYOFF = [
    BLANK,
    BLANK,
    "RAILYARD".center(COLS),
    BLANK,
    "ONE API CALL. INSTANT SETTLEMENT.".center(COLS),
    "A WEBHOOK AT EVERY STATE CHANGE.".center(COLS),
    BLANK,
]
for row in OLD + PAYOFF + [CALL, NEW("SETTLED"), hook_row("SETTLED")]:
    assert len(row) == COLS, (row, len(row))


def colour_of(r, c, text, phase):
    """Ink colour for a resolved character (status column semantics)."""
    if phase == "payoff":
        return GREEN if r == 2 else INK
    if c >= 31:
        word = text[31:].strip()
        if word in ("DELAYED", "QUEUED", "HELD"):
            return AMBER
        if word == "SETTLED":
            return GREEN
    if r == 5 and text.strip().endswith("SETTLED") and c >= 9:
        return GREEN if c >= 28 else INK
    return INK


# ---- schedule of changes: (time, row, newtext, phase, jitter, random_start) --
events = []
# Hook: board rows 0-3 are mid-clatter at frame 0 (start from random flaps).
for r in range(4):
    events.append((0.0, r, OLD[r], "old", 0.10, True))
events.append((4.0, 4, CALL, "call", 0.05, False))
events.append((6.0, 4, NEW("CREATED"), "new", 0.04, False))
events.append((6.45, 5, hook_row("CREATED"), "new", 0.03, False))
events.append((7.25, 4, NEW("PENDING"), "new", 0.0, False))
events.append((7.35, 5, hook_row("PENDING"), "new", 0.0, False))
events.append((8.25, 4, NEW("SETTLED"), "new", 0.0, False))
events.append((8.35, 5, hook_row("SETTLED"), "new", 0.0, False))
for r in range(ROWS):
    events.append((10.0 + r * 0.035, r, PAYOFF[r], "payoff", 0.06, False))
events.append((13.3, 4, BLANK, "payoff", 0.05, False))
events.append((13.3, 5, BLANK, "payoff", 0.05, False))
events.append((13.4, 3, "DELAYS: NONE".center(COLS), "payoff", 0.03, False))

# ---- simulate every cell ---------------------------------------------------
# For each cell: list of glyph spans (char, start, end, colour) and flip windows.
spans = {(r, c): [] for r in range(ROWS) for c in range(COLS)}
flips = {(r, c): [] for r in range(ROWS) for c in range(COLS)}
state = {(r, c): (" ", 0.0, INK) for r in range(ROWS) for c in range(COLS)}  # char, since, colour
clatter = []   # (time, count) per event for sound
END = 15.0


def advance(r, c, target, t0, col, rand_start):
    ch, since, ccol = state[(r, c)]
    if rand_start:
        ch = random.choice(ALPHA[1:])
    a, b = ALPHA.index(ch), ALPHA.index(target)
    steps = (b - a) % len(ALPHA)
    if steps == 0 and not rand_start:
        if col != ccol:
            spans[(r, c)].append((ch, since, t0, ccol))
            state[(r, c)] = (ch, t0, col)
        return None
    if rand_start and steps == 0:
        steps = len(ALPHA)
    skip = max(0, steps - MAXSTEPS)
    if skip:
        a = (a + skip) % len(ALPHA)
        steps -= skip
    # close previous span
    if since < t0 and ch != " " and not rand_start:
        spans[(r, c)].append((state[(r, c)][0], since, t0, ccol))
    t = t0
    for k in range(1, steps + 1):
        g = ALPHA[(a + k) % len(ALPHA)]
        last = k == steps
        t1 = t + FLIP
        gc = col if last else INK
        if g != " " and not last:
            spans[(r, c)].append((g, t, t1, gc))
        t = t1
    flips[(r, c)].append((t0, t))
    state[(r, c)] = (target, t, col)
    return t


for (t0, r, text, phase, jit, rnd) in sorted(events, key=lambda e: e[0]):
    stops = []
    for c in range(COLS):
        start = t0 + random.uniform(0, jit)
        col = colour_of(r, c, text, phase)
        tend = advance(r, c, text[c], start, col, rnd and text[c] != " " or (rnd and random.random() < 0.5))
        if tend:
            stops.append(tend)
    if stops:
        clatter.append((t0, max(stops), len(stops)))

for (r, c), (ch, since, col) in state.items():
    if ch != " ":
        spans[(r, c)].append((ch, since, END, col))

# ---- layers ----------------------------------------------------------------
L = []
# Housing: enamel ground, warm practical light from above, panel edges.
L.append({"shape": "rect", "width": W + 400, "height": H + 400, "fill": ENAMEL})
L.append({"shape": "rect", "width": 1840, "height": 900, "x": 960, "y": 548, "radius": 10,
          "fill": "linear-gradient(180deg, #161B18, #0E1210 60%, #0A0D0B)",
          "stroke": "#26302A", "strokeWidth": 2})
# printed header plate
L.append({"text": "TRANSFERS", "font": "Inter Display", "weight": 700, "size": 30, "letterSpacing": "0.32em",
          "color": INK, "x": X0, "y": 128, "anchor": "left"})
L.append({"text": "DEPARTURES · ARRIVALS", "font": "Inter Display", "weight": 500, "size": 20,
          "letterSpacing": "0.3em", "color": LABEL, "x": X0 + 300, "y": 129, "anchor": "left"})
for name, c in (("SENT", 0), ("TRANSFER", 9), ("ARRIVES", 22), ("STATUS", 31)):
    L.append({"text": name, "font": "Inter Display", "weight": 600, "size": 17, "letterSpacing": "0.28em",
              "color": LABEL, "x": X0 + c * PX + 2, "y": Y0 - 26, "anchor": "left"})
L.append({"shape": "rect", "width": COLS * PX + 20, "height": 2, "fill": "#2A332D", "x": 960, "y": Y0 - 10})
# screws
for sx, sy in ((70, 128), (1850, 128), (70, 968), (1850, 968)):
    L.append({"shape": "circle", "width": 16, "height": 16, "fill": "radial-gradient(#4a524c, #1a1f1c)", "x": sx, "y": sy})

# clock (top right): 8 small flaps
CLK = {"x0": 1846 - 8 * 36 + 4, "y": 104, "pw": 36, "cw": 32, "ch": 50}
clock_states = [(0.0, "09:41:06"), (6.0, "09:41:07")]
L.append({"text": "TIME", "font": "Inter Display", "weight": 600, "size": 15, "letterSpacing": "0.28em",
          "color": LABEL, "x": CLK["x0"] - 16, "y": CLK["y"] + 25, "anchor": "right"})


def flap_cell(x, y, w, h, out):
    out.append({"shape": "rect", "width": w, "height": h / 2 - 1, "x": x, "y": y, "anchor": "topLeft", "radius": 3,
                "fill": f"linear-gradient(180deg, #2B2E2C, {FLAP_T})"})
    out.append({"shape": "rect", "width": w, "height": h / 2 - 1, "x": x, "y": y + h / 2 + 1, "anchor": "topLeft",
                "radius": 3, "fill": f"linear-gradient(180deg, {FLAP_B}, #151716)"})


def glyph(ch, x, y, w, h, size, col, start, end):
    lay = {"text": ch, "font": "Inter Display", "weight": 600, "size": size, "color": col,
           "x": x + w / 2, "y": y + h / 2 + 1, "start": round(start, 4)}
    if end < END - 1e-6:
        lay["end"] = round(end, 4)
    return lay


def hinge(x, y, w, h, out):
    out.append({"shape": "rect", "width": w, "height": 2, "x": x, "y": y + h / 2 - 1, "anchor": "topLeft",
                "fill": "#050606"})
    # side pins
    out.append({"shape": "rect", "width": 3, "height": 8, "x": x - 2, "y": y + h / 2 - 4, "anchor": "topLeft", "fill": "#3a3f3b"})
    out.append({"shape": "rect", "width": 3, "height": 8, "x": x + w - 1, "y": y + h / 2 - 4, "anchor": "topLeft", "fill": "#3a3f3b"})


def flap_motion(x, y, w, h, a, b, out):
    # the falling top flap: shrinks toward the hinge each FLIP, carrying a darker face
    period = FLIP
    out.append({"shape": "rect", "width": w, "height": h / 2 - 1, "x": x, "y": y + h / 2 - 1, "anchor": "bottomLeft",
                "radius": 3, "fill": "#2F3331", "start": round(a, 4), "end": round(b, 4),
                "scaleY": f"=1 - fract(t/{period})", "opacity": 0.92})
    out.append({"shape": "rect", "width": w, "height": h / 2 - 1, "x": x, "y": y + h / 2 + 1, "anchor": "topLeft",
                "radius": 3, "fill": "#101211", "start": round(a, 4), "end": round(b, 4),
                "scaleY": f"=fract(t/{period})", "opacity": 0.85})


board = []
for r in range(ROWS):
    for c in range(COLS):
        x, y = cell_xy(r, c)
        flap_cell(x, y, CW, CH, board)
for r in range(ROWS):
    for c in range(COLS):
        x, y = cell_xy(r, c)
        for (ch, s, e, col) in spans[(r, c)]:
            if e - s > 1e-4:
                board.append(glyph(ch, x, y, CW, CH, 56, col, s, e))
for r in range(ROWS):
    for c in range(COLS):
        x, y = cell_xy(r, c)
        hinge(x, y, CW, CH, board)
        for (a, b) in flips[(r, c)]:
            flap_motion(x, y, CW, CH, a, b, board)

# clock cells
cs = {}
for i in range(8):
    x = CLK["x0"] + i * CLK["pw"]
    flap_cell(x, CLK["y"], CLK["cw"], CLK["ch"], board)
for i in range(8):
    x = CLK["x0"] + i * CLK["pw"]
    seq = []
    for k, (t, s) in enumerate(clock_states):
        t_end = clock_states[k + 1][0] if k + 1 < len(clock_states) else END
        seq.append((s[i], t, t_end))
    merged = []
    for ch, a, b in seq:
        if merged and merged[-1][0] == ch:
            merged[-1] = (ch, merged[-1][1], b)
        else:
            merged.append((ch, a, b))
    for ch, a, b in merged:
        a2 = a + (0.0 if a == 0 else 0.07)
        board.append(glyph(ch, x, CLK["y"], CLK["cw"], CLK["ch"], 34, INK, a2, b))
    hinge(x, CLK["y"], CLK["cw"], CLK["ch"], board)
    for k, (ch, a, b) in enumerate(merged[1:]):
        flap_motion(x, CLK["y"], CLK["cw"], CLK["ch"], a, a + 0.07, board)

L.extend(board)

# status lamps (right of each row, outside the flaps)
LAMP_X = X0 + COLS * PX + 14
for r in range(4):
    _, y = cell_xy(r, 0)
    L.append({"shape": "circle", "width": 12, "height": 12, "x": LAMP_X, "y": y + CH / 2, "fill": AMBER,
              "glow": AMBER, "opacity": "=0.55 + 0.45*step(0.5, fract(gt*0.9 + " + str(r * 0.27) + "))",
              "end": 10.0})
_, y4 = cell_xy(4, 0)
L.append({"shape": "circle", "width": 12, "height": 12, "x": LAMP_X, "y": y4 + CH / 2, "fill": "#F4EFE6", "glow": "#F4EFE6",
          "start": 6.4, "end": 8.85, "opacity": "=0.6 + 0.4*step(0.5, fract(gt*2))"})
L.append({"shape": "circle", "width": 12, "height": 12, "x": LAMP_X, "y": y4 + CH / 2, "fill": GREEN, "glow": GREEN,
          "start": 8.85, "end": 10.0})
_, y2 = cell_xy(2, 0)
L.append({"shape": "circle", "width": 12, "height": 12, "x": LAMP_X, "y": y2 + CH / 2, "fill": GREEN, "glow": GREEN,
          "start": 10.5})

# Light: warm practical from above + floor falloff; the dispatcher's lamp narrows on rows 4-5 for the turn.
L.append({"shape": "rect", "width": W + 400, "height": H + 400,
          "fill": "radial-gradient(circle at 50% 0%, rgba(255,214,160,0.10), rgba(0,0,0,0) 60%)", "blend": "screen"})
L.append({"shape": "rect", "width": W + 400, "height": H + 400,
          "fill": "linear-gradient(180deg, rgba(0,0,0,0) 55%, rgba(0,0,0,0.35))"})
spot_rows = Y0 + 4 * PY + CH  # centre between rows 4 and 5
L.append({"shape": "rect", "width": W + 400, "height": H + 400,
          "fill": f"linear-gradient(180deg, rgba(0,0,0,0.78) 0%, rgba(0,0,0,0.78) 52%, rgba(0,0,0,0) 61%, rgba(0,0,0,0) 81%, rgba(0,0,0,0.78) 88%)",
          "opacity": 0, "animate": [
              {"to": {"opacity": 1}, "at": 5.9, "duration": 0.5, "ease": "outCubic"},
              {"to": {"opacity": 0}, "at": 9.8, "duration": 0.45, "ease": "inOutCubic"}]})

# ---- camera ----------------------------------------------------------------
def cam(px, py):
    return {"x": px - W / 2, "y": py - H / 2}

hx, hy = cell_xy(0, 34)
hook = {"zoom": 3.4, **cam(hx + CW / 2 - 40, hy + CH / 2)}
wide = {"zoom": 1.0, **cam(960, 540)}
row4 = {"zoom": 1.06, **cam(960, Y0 + 4 * PY + CH)}
pay = {"zoom": 1.08, **cam(960, Y0 + 3.5 * PY + 8)}
camera = {"animate": [
    {"from": hook, "to": {"zoom": 2.9, "x": hook["x"] - 60, "y": hook["y"]}, "duration": 1.05, "ease": "linear"},
    {"to": wide, "at": 1.05, "duration": 2.6, "ease": "inOutQuart"},
    {"to": row4, "at": 5.95, "duration": 3.4, "ease": "outCubic"},
    {"to": pay, "at": 9.9, "duration": 1.6, "ease": "inOutCubic"},
    {"to": {"zoom": 2.0, **cam(960, Y0 + 2.5 * PY + CH / 2)}, "at": 12.7, "duration": 1.0, "ease": "inOutQuart"},
]}

# ---- sound -----------------------------------------------------------------
audio = []
for (a, b, n) in clatter:
    k = a
    dens = 0.022 if n > 20 else 0.04
    while k < b:
        audio.append({"sfx": "tick", "at": round(k, 3), "volume": f"-{random.randint(14, 22)}dB",
                      "pitch": round(random.uniform(0.85, 1.35), 2)})
        k += dens * random.uniform(0.6, 1.4)
    audio.append({"sfx": "click", "at": round(b, 3), "volume": "-10dB", "pitch": 0.8})
# clock flips
for t in (6.0,):
    audio.append({"sfx": "click", "at": t, "volume": "-6dB", "pitch": 0.7})
    audio.append({"sfx": "tick", "at": t + 0.035, "volume": "-9dB", "pitch": 0.9})
# station chime for every webhook
for t, p in ((6.75, 1.0), (7.75, 1.12), (8.75, 1.26)):
    audio.append({"sfx": "chime", "at": t, "volume": "-7dB", "pitch": p})
audio.append({"sfx": "ding", "at": 13.75, "volume": "-6dB", "pitch": 1.0})

music = {
    "bpm": 120, "key": "Dm", "length": "15s", "end": "ring", "master": {"lufs": -15},
    "hits": ["@6.0", "@10.0"],
    "tracks": [
        {"inst": "piano", "name": "pulse", "notes": "l8 D2 r r D2 r r D2 r | D2 r r D2 r r D2 r | Bb1 r r Bb1 r r Bb1 r | F2 r r F2 r r F2 r | A1 r r A1 r r C#2 r | D2/1", "gain": -9, "reverb": 0.3},
        {"inst": "piano", "notes": "@4 l2 [F3 A3 D4] [E3 G3 C4] | [G3 Bb3 D4] [A3 C#4 E4] | [D3 F#3 A3 D4]/1 | [D3 F#3 A3 D4]/1 | [D3 F#3 A3]/2", "gain": -8, "reverb": 0.5},
        {"inst": "strings", "notes": "@4 l1 [D3 A3] | [G2 D3] | [D3 A3 D4 F#4] | [D3 A3 D4 F#4] | [D3 A3 D4 F#4]/2", "gain": -11, "reverb": 0.5},
        {"inst": "bass", "notes": "@3 l4 D2 r D2 r | F2 r E2 r | G1 r A1 r | D2/1 | D2/1 | D2/2", "gain": -8},
        {"inst": "glockenspiel", "notes": "@4 l8 r r r A5 r r r B5 | r r r C#6 r r r r | r r r r r r r r | r r r r r r r D6/2", "gain": -10, "reverb": 0.5},
        {"inst": "drums:brush", "drums": {"hat": "x.x.x.x.x.x.x.x.", "kick": "x.......x......."}, "at": "@bar3", "bars": 3, "gain": -14},
    ],
}

spec = {
    "size": "16:9",
    "fps": 30,
    "background": ENAMEL,
    "intent": {"pace": "steady", "genre": "launch"},
    "music": music,
    "audio": audio,
    "post": {"vignette": 0.32, "grain": 0.018, "contrast": 1.04, "temperature": 0.04},
    "scenes": [{"duration": 15, "camera": camera, "layers": L}],
}
out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "spec.json")
json.dump(spec, open(out, "w"), indent=None, separators=(",", ":"))
print("layers", len(L), "audio", len(audio))
