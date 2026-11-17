#!/usr/bin/env python3
"""Generates spec.json: Railyard launch film. A railway signal-box panel where a
payment is a lit train and every state change fires a webhook down a spur."""
import json, random, os
random.seed(7)

PANEL = "#16201C"; SEAM = "#0D1311"; RAIL = "#D9D3C2"; RAIL_DIM = "#56645F"
AMBER = "#FFB13B"; GREEN = "#3DF58E"; RED = "#FF4433"; CREAM = "#EEE7D4"; INK = "#0E1311"
MONO = "JetBrains Mono"; DISP = "Inter Display"
MAIN_Y = 540
L = []  # layers

def rect(x, y, w, h, fill, anchor="center", **k):
    d = {"shape": "rect", "x": x, "y": y, "width": w, "height": h, "fill": fill, "anchor": anchor}
    d.update(k); return d

def path(pts, stroke, sw, **k):
    xs = [p[0] for p in pts]; ys = [p[1] for p in pts]
    d = "M" + " L".join(f"{x} {y}" for x, y in pts)
    o = {"shape": "path", "d": d, "stroke": stroke, "strokeWidth": sw, "fill": "none",
         "x": min(xs) - sw / 2 if False else min(xs), "y": min(ys), "anchor": "topLeft"}
    o.update(k); return o

def text(s, x, y, size, color, font=MONO, anchor="left", **k):
    o = {"text": s, "font": font, "size": size, "color": color, "x": x, "y": y, "anchor": anchor}
    o.update(k); return o

def lamp(x, y, d, tf, fade=""):
    off = f"=(gt<{tf} ? 1 : 0){fade}"; on = f"=(gt<{tf} ? 0 : 1){fade}"
    return [{"shape": "circle", "width": d, "height": d, "x": x, "y": y, "fill": RED, "glow": RED, "opacity": off},
            {"shape": "circle", "width": d, "height": d, "x": x, "y": y, "fill": GREEN, "glow": GREEN, "opacity": on}]

def step(tf, a, b):  # colour switch at video time tf
    return f"=gt<{tf} ? {a} : {b}"

# ---------- the panel: tile seams ----------
L.append(rect(960, 400, 5200, 3400, PANEL))
L.append({"shape": "rect", "x": 960, "y": 400, "width": 5200, "height": 3400,
          "fill": "radial-gradient(#22302A, #101714)", "opacity": 0.85})
for i in range(-17, 42):   # vertical seams every 120
    L.append(rect(-1240 + i * 120 + 2040, 400, 2, 3400, SEAM))
for j in range(0, 29):
    L.append(rect(960, -1300 + j * 120, 5200, 2, SEAM))

# ---------- yard: tracks, joints, crossovers ----------
YARD = [k for k in range(-6, 5) if k != 0]
yard_op = "=0.45 + 0.55*smoothstep(7.4, 8.6, gt) - 0.65*smoothstep(11.5, 12.3, gt)"
for k in YARD:
    y = MAIN_Y + 160 * k
    L.append(rect(-1500, y, 5000, 5, RAIL_DIM, anchor="left", opacity=yard_op))
    for _ in range(14):  # insulated joints (track-circuit gaps)
        L.append(rect(random.uniform(-1400, 3400), y, 6, 14, PANEL))
for k in YARD + [0]:
    if k + 1 > 5 or k + 1 == 0 and False: pass
    if k == 5: continue
    y0 = MAIN_Y + 160 * k
    for _ in range(3):
        x0 = random.uniform(-1200, 3200)
        if k in (-1, 0) and -300 < x0 < 2300: continue  # keep the hero line clean
        L.append(path([(x0, y0), (x0 + 160, y0 + 160)], RAIL_DIM, 5, opacity=yard_op))

# ---------- yard trains and their webhooks ----------
SPAN = 5000
FADE = "*(1 - 0.75*smoothstep(11.5, 12.3, gt))"
amounts = ["$840.00", "$2,150", "$96.40", "$18,000", "$3,275", "$610.12", "$12.99", "$7,400", "$455.00",
           "$1,020", "$58.30", "$9,999", "$240.00", "$31,500", "$77.10", "$5,600", "$1,288", "$302.75"]
events = ["transfer.created", "transfer.pending", "transfer.settled"]
ai = 0
for k in YARD:
    y = MAIN_Y + 160 * k
    for n in range(3):
        v = random.uniform(160, 420) * random.choice([1, -1])
        off = random.uniform(0, SPAN)
        if v > 0: xe = f"-1500 + mod({off:.0f} + {v:.0f}*gt, {SPAN})"
        else:     xe = f"3500 - mod({off:.0f} + {-v:.0f}*gt, {SPAN})"
        amt = amounts[ai % len(amounts)]; ai += 1
        L.append({"layers": [
            rect(0, 0, 120, 11, AMBER, glow=AMBER),
            rect(0, -34, 112, 32, INK, stroke="#3A4642", strokeWidth=2, radius=3),
            text(amt, 0, -34, 18, AMBER, anchor="center")],
            "x": "=" + xe, "y": y, "anchor": "topLeft", "opacity": yard_op})
        # a signal the train passes during the reveal: lamp flips, webhook fires
        tf = random.uniform(8.1, 11.2)
        sx = (-1500 + (off + v * tf) % SPAN) if v > 0 else (3500 - (off - v * tf) % SPAN)
        if not (-950 < sx < 2850): continue
        if k in (-1, 1) and -450 < sx < 2450: continue
        sx = round(sx, 1); tf = round(tf, 2)
        dirn = 1 if v > 0 else -1
        ev = random.choice(events)
        L.append(path([(sx, y), (sx + 50 * dirn, y - 50), (sx + 110 * dirn, y - 50)], RAIL_DIM, 4, opacity=yard_op))
        L.extend(lamp(sx - 22 * dirn, y + 22, 18, tf, FADE))
        px = sx + 110 * dirn
        L.append(text(ev, px + 8 * dirn, y - 50, 24, step(tf, "#5E6C67", CREAM),
                      anchor="left" if dirn > 0 else "right", opacity="=1" + FADE))
        L.append({"shape": "circle", "width": 110, "height": 110, "x": px, "y": y - 50, "fill": "none",
                  "stroke": GREEN, "strokeWidth": 3,
                  "scale": f"=0.2 + 1.6*clamp((gt-{tf})/0.5, 0, 1)",
                  "opacity": f"=gt<{tf} ? 0 : max(0, 1 - (gt-{tf})/0.5)"})

# ---------- the hero line ----------
A_X, B_X = 40, 1960
L.append(rect(-150, MAIN_Y, 2260, 6, RAIL, anchor="left"))
# buffer stops
for bx, s in ((-150, 1), (2110, -1)):
    L.append(rect(bx, MAIN_Y, 8, 46, RAIL))
# route set: amber light runs ahead of the money, turns green once settled
L.append(rect(A_X - 60, MAIN_Y, B_X + 40 - A_X, 8, GREEN, anchor="left", glow=GREEN,
              opacity="=smoothstep(5.98, 6.1, gt)"))
L.append(rect(A_X - 60, MAIN_Y, 0, 8, AMBER, anchor="left", glow=AMBER, opacity="=gt<6.05 ? 1 : 0",
              animate=[{"from": {"width": 0}, "to": {"width": B_X + 40 - A_X}, "at": 0.35, "duration": 1.1, "ease": "inOutExpo"}]))

# signals: (x, fire time, event)
SIG = [(320, 2.0, "transfer.created"), (1040, 4.0, "transfer.pending"), (1880, 6.0, "transfer.settled")]
for sx, tf, ev in SIG:
    L.append(path([(sx, MAIN_Y), (sx + 60, MAIN_Y - 60), (sx + 120, MAIN_Y - 60)], RAIL, 5))
    # lamp + post
    L.append(rect(sx - 34, MAIN_Y + 28, 3, 20, RAIL_DIM))
    L.extend(lamp(sx - 34, MAIN_Y + 40, 22, tf))
    # webhook plate
    L.append(rect(sx + 120, MAIN_Y - 60, 400, 58, INK, anchor="left", radius=4,
                  stroke=step(tf, "#33403C", GREEN), strokeWidth=2))
    L.append(text("POST /webhooks", sx + 140, MAIN_Y - 78, 15, step(tf, "#4E5B57", "#9FB0A9")))
    L.append(text(ev, sx + 140, MAIN_Y - 52, 27, step(tf, "#5E6C67", CREAM)))
    L.append(text("200", sx + 500, MAIN_Y - 60, 22, GREEN, anchor="right",
                  opacity=f"=gt<{tf}+0.3 ? 0 : 1"))
    # pulse travelling up the spur
    u = f"clamp((gt-{tf})/0.3, 0, 1)"
    L.append({"shape": "circle", "width": 16, "height": 16, "fill": "#FFFFFF", "glow": GREEN,
              "x": f"={sx} + 120*{u}", "y": f"={MAIN_Y} - 60*min({u}*2, 1)",
              "opacity": f"=gt>{tf} && gt<{tf}+0.32 ? 1 : 0"})
    L.append({"shape": "circle", "width": 90, "height": 90, "x": sx + 120, "y": MAIN_Y - 60, "fill": "none",
              "stroke": GREEN, "strokeWidth": 3,
              "scale": f"=0.2 + 1.5*clamp((gt-{tf}-0.3)/0.5, 0, 1)",
              "opacity": f"=gt<{tf}+0.3 ? 0 : max(0, 1 - (gt-{tf}-0.3)/0.5)"})

# terminus plates
L.append(rect(-170, MAIN_Y + 60, 0, 0, INK))
L.append(text("BANK A", -130, MAIN_Y + 54, 26, CREAM, font=DISP, weight=700, letterSpacing="0.12em"))
L.append(text("acct ••4821", -130, MAIN_Y + 86, 18, "#9FB0A9"))
L.append(text("BANK B", 2140, MAIN_Y + 4, 26, CREAM, font=DISP, weight=700, letterSpacing="0.12em"))
L.append(text("acct ••0937", 2140, MAIN_Y + 36, 18, "#9FB0A9"))
L.append(text("+$12,400.00", 2140, MAIN_Y + 76, 30, GREEN, opacity="=gt<6.1 ? 0 : 1"))

# the API call: train-describer LED that types the request
L.append(rect(-150, MAIN_Y - 96, 560, 62, INK, anchor="left", radius=4, stroke="#3A4642", strokeWidth=2))
L.append(text("POST /v1/transfers", -128, MAIN_Y - 96, 34, AMBER, start=0.05,
              enter={"preset": "type", "duration": 0.7}))
L.append({"shape": "rect", "x": 390, "y": MAIN_Y - 96, "width": 4, "height": 34, "fill": AMBER,
          "opacity": "=gt<1.2 ? (fract(gt*3)<0.5 ? 1 : 0) : 0"})

# the money: a lit train with its describer, moving with weight
tx = ("=gt<1.5 ? 40 : gt<2 ? 40+280*pow((gt-1.5)/0.5,2) : gt<4 ? 320+720*(1-pow(1-(gt-2)/2,2)) : "
      "gt<5.5 ? 1040 : gt<6 ? 1040+840*pow((gt-5.5)/0.5,3) : 1880 + 30*(1-exp(-(gt-6)*9))*sin(min(gt-6,0.35)*9)")
L.append({"layers": [
    rect(0, 0, 150, 14, "#FFE2A8", glow=AMBER),
    rect(0, 42, 190, 40, INK, stroke=AMBER, strokeWidth=2, radius=4),
    text("$12,400.00", 0, 42, 24, AMBER, anchor="center")],
    "x": tx, "y": MAIN_Y, "anchor": "topLeft", "start": 0.6,
    "enter": {"preset": "pop", "duration": 0.35}})

# Settled instantly.
L.append(rect(1420, 800, 1000, 132, INK, anchor="left", radius=6, start=6.15,
              enter={"preset": "wipe", "duration": 0.3}))
L.append(text("Settled instantly.", 1460, 800, 76, CREAM, font=DISP, weight=800, letterSpacing="-0.02em",
              start=6.2, enter="mask by line up"))

# reveal headline plate at the bottom of the panel
L.append(rect(960, 1390, 2600, 220, CREAM, radius=8, start=8.3, enter={"preset": "wipe", "duration": 0.4}))
L.append(text("A webhook at every state change.", 960, 1390, 132, INK, font=DISP, weight=800, width=2500, align="center",
              anchor="center", letterSpacing="-0.02em", start=8.45, enter="mask by line up"))

# nameplate (the logo) at the top of the panel
NY = -800
L.append(rect(960, NY, 1500, 330, CREAM, radius=10))
L.append(rect(960, NY, 1470, 300, "none", radius=8, stroke=INK, strokeWidth=3))
# mark: two rails, a lit segment, a green lamp
L.append(rect(470, NY - 40, 150, 10, INK))
L.append(rect(470, NY + 4, 150, 10, INK))
L.append(rect(470, NY - 18, 80, 14, "#B9B29F"))
L.append(rect(470, NY - 18, 80, 14, AMBER, glow=AMBER, opacity="=smoothstep(12.25, 12.35, gt)"))
L.append({"shape": "circle", "width": 26, "height": 26, "x": 560, "y": NY + 36, "fill": "#8A2A22"})
L.append({"shape": "circle", "width": 26, "height": 26, "x": 560, "y": NY + 36, "fill": GREEN, "glow": GREEN,
          "opacity": "=smoothstep(12.25, 12.35, gt)"})
L.append(text("Railyard", 600, NY - 22, 170, INK, font=DISP, weight=800, letterSpacing="-0.035em"))
L.append(text("One API call moves money between bank accounts.", 610, NY + 96, 34, "#3A3F3B",
              start=12.2, enter={"preset": "type", "duration": 0.6}))
L.append(text("PAYMENTS API", 260, NY - 112, 20, "#6A6A60", letterSpacing="0.3em"))

def C(x, y):  # camera pan for a world centre
    return {"x": x - 960, "y": y - 540}

cam = {"zoom": 2.7, **C(130, 500), "animate": [
    {"to": {"zoom": 2.35, **C(180, 505)}, "duration": 1.5, "ease": "inOutSine"},
    {"to": {"zoom": 2.0, **C(420, 500)}, "at": 1.5, "duration": 0.7, "ease": "inOutCubic"},
    {"to": {"zoom": 2.0, **C(1120, 500)}, "at": 2.2, "duration": 1.9, "ease": "inOutSine"},
    {"to": {"zoom": 2.35, **C(1180, 505)}, "at": 4.1, "duration": 1.4, "ease": "inQuad"},
    {"to": {"zoom": 1.65, **C(1930, 560)}, "at": 5.5, "duration": 0.5, "ease": "inOutExpo"},
    {"to": {"zoom": 1.55, **C(1960, 580)}, "at": 6.0, "duration": 1.7, "ease": "outSine"},
    {"to": {"zoom": 0.5, **C(960, 450)}, "at": 7.7, "duration": 1.5, "ease": "inOutExpo"},
    {"to": {"zoom": 0.53, **C(960, 430)}, "at": 9.2, "duration": 2.2, "ease": "linear"},
    {"to": {"zoom": 1.12, **C(960, NY + 20)}, "at": 11.4, "duration": 0.9, "ease": "inOutExpo"},
    {"to": {"zoom": 1.18, **C(960, NY + 20)}, "at": 12.3, "duration": 2.7, "ease": "outSine"}],
    "shake": {"amount": 9, "frequency": 14, "at": 6.0, "duration": 0.35}}

spec = {
  "size": "16:9", "fps": 30, "background": PANEL,
  "intent": {"pace": "steady", "genre": "launch"},
  "motionBlur": {"shutter": 60},
  "post": {"bloom": 0.25, "bloomThreshold": 0.85, "vignette": 0.35, "grain": 0.025, "contrast": 1.05},
  "music": {"bpm": 120, "key": "Dm", "end": "ring", "followCuts": False,
    "tracks": [
      {"inst": "cello", "notes": "l8 (D2 D2 D2 D2 D2 D2 D2 D2 | D2 D2 D2 D2 D2 D2 D2 D2 | Bb1 Bb1 Bb1 Bb1 Bb1 Bb1 Bb1 Bb1 | C2 C2 C2 C2 C2 C2 C2 C2)x2", "gain": -3},
      {"inst": "strings", "notes": "@3 o4 [D4 F4 A4]/1 | [Bb3 D4 F4]/2 [C4 E4 G4]/2 | @4 [D4 F4 A4]/1 | [Bb3 D4 F4]/1 | [G3 Bb3 D4]/1 | [A3 C#4 E4]/1 | [D4 F#4 A4]/1 | [D4 F#4 A4]/1", "gain": -6, "reverb": 0.35},
      {"inst": "marimba", "notes": "o5 l16 (D A F A D A F A D A F A D A F A)x3 | @4 (D A F A D A F A D A F A D A F A)x3", "gain": -10, "pan": 0.25},
      {"inst": "timpani", "notes": "@4 o2 D2/4! r/4 r/2 | @7 D2/1!", "gain": -2},
      {"inst": "french-horn", "notes": "@4 o4 l4 D4 A4 F4/2 | G4/2 A4/2 | Bb4 A4 G4/2 | A4/1 | @7 [D4 F#4 A4]/1", "gain": -5, "reverb": 0.3},
      {"inst": "drums:orchestra", "drums": {"kick": "x.......x.......", "snare": "....x.......x.x."}, "at": "@bar4", "bars": 3, "gain": -6}
    ],
    "sfx": [{"sfx": "click", "at": 0.15}, {"sfx": "typing", "at": 0.2, "gain": -10},
            {"sfx": "tick", "at": 2.0}, {"sfx": "tick", "at": 4.0},
            {"sfx": "riser", "at": 5.95}, {"sfx": "impact", "at": 6.0}, {"sfx": "ding", "at": 6.3, "gain": -6},
            {"sfx": "whoosh", "at": 8.1}, {"sfx": "whoosh", "at": 11.85}, {"sfx": "chime", "at": 12.3}]},
  "scenes": [{"duration": 15, "camera": cam, "layers": L}],
}
out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "spec.json")
json.dump(spec, open(out, "w"), indent=1)
print(out, len(L), "layers")
