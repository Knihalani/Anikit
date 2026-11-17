"""Builds vertical.json: a ~15 s 9:16 social cut-down of "Telemetry".

    python3 examples/video/telemetry/vertical.py   # writes vertical.json (and race.json)

Same race simulation and the same scene code as spec.py, re-laid out for
1080×1920: the Turn 1 pass, the fastest lap and the winner. Text stays inside
the 9:16 safe area (top 12% and bottom 20% clear); only decoration bleeds.
The score is the film's score.mp3, re-cut on the new scene times.
"""

import json
import os

import spec as S
from spec import (BG, COL, DATA, HEAD, MONO, MUTED, NAME, PANEL, PURPLE, RED, TEAM, TEXT, YELLOW,
                  group, r, rect, text)

W, H = 1080, 1920
S.W, S.H = W, H  # the shared scene code reads the frame size from these
M = S.M
TOP, BOTTOM = int(H * 0.12), int(H * 0.80)  # safe band for text: 230..1536


def overtake():
    sc = S.scene_overtake({
        "zx": 40, "zy": 40, "card_x": M + 26, "card_y": TOP + 90, "card_w": W - 2 * M - 52,
        "banner_x": W / 2, "banner_y": 1290, "banner_w": 900,
        "cap_y": 1400, "tag_y": TOP + 10, "slow_y": BOTTOM - 30, "slow_x": W - M - 40,
    })
    sc["duration"] = 6.0
    return sc


def fastest():
    dur = 3.6
    fl = DATA["fastest_lap"]
    code = fl["code"]
    secs = DATA["sector_times"][code][str(fl["lap"])]
    mins = int(fl["time"] // 60)
    secs_v = fl["time"] - 60 * mins
    layers = [{"shape": "rect", "width": W, "height": H, "x": W / 2, "y": H / 2,
               "fill": "linear-gradient(160deg, #1A0633, #05070B)"}]
    for k in range(9):  # speed streaks
        y = 160 + k * 190
        layers.append(rect(-400, y, 600 + 110 * (k % 3), 6 + 4 * (k % 2), PURPLE, opacity=0.3 + 0.1 * (k % 3), anchor="left",
                           animate=[{"from": {"x": -900}, "to": {"x": W + 300}, "duration": 1.1 + 0.15 * (k % 4),
                                     "at": 0.05 * k, "ease": "inOutCubic"}]))
    x0 = M + 30
    layers += [
        rect(M, 470, 14, 560, PURPLE, enter={"preset": "wipeIn", "direction": "down", "duration": 0.4}),
        text("FASTEST", x0, 540, 150, TEXT, HEAD, 900, "left", italic=True, split="chars",
             enter={"preset": "fadeLeft", "stagger": 0.025, "duration": 0.4, "ease": "outExpo"}),
        text("LAP", x0, 690, 150, TEXT, HEAD, 900, "left", italic=True, split="chars",
             enter={"preset": "fadeLeft", "stagger": 0.025, "delay": 0.1, "duration": 0.4, "ease": "outExpo"}),
        text(f"{mins}:{{value:00.000}}", x0 + 4, 860, 150, PURPLE, MONO, 800, "left", value=r(secs_v, 3),
             glow={"radius": 30, "intensity": 0.9, "color": PURPLE},
             animate=[{"from": {"value": r(secs_v - 6.5, 3)}, "at": 0.1, "duration": 0.45, "ease": "outExpo"}]),
        text(f"{NAME[code].upper()}  ·  LAP {fl['lap']}", x0 + 6, 980, 34, TEXT, MONO, 700, "left",
             letterSpacing="0.06em", enter={"preset": "fadeUp", "delay": 0.4}),
    ]
    for k, s in enumerate(secs):
        y = 1100 + k * 112
        layers.append(group([
            rect(M, y, W - 2 * M, 92, "#140A26"),
            rect(M, y, 10, 92, PURPLE),
            text(f"S{k + 1}", M + 40, y + 46, 34, MUTED, MONO, 800, "left"),
            text(f"{s:.3f}", W - M - 30, y + 46, 54, TEXT, MONO, 800, "right"),
        ], enter={"preset": "fadeLeft", "delay": 0.5 + 0.12 * k, "duration": 0.5, "ease": "outExpo"}))
    layers.append({"component": "sparks", "props": {"bursts": [0.3], "origin": [0.4, 0.44], "count": 90,
                                                    "colors": ["#FFFFFF", PURPLE], "power": 1.1}})
    return {"id": "fastest", "duration": dur, "background": BG,
            "transition": {"type": "glitch", "duration": 0.25, "intensity": 0.6}, "layers": layers}


def winner():
    dur = 6.0
    cl = DATA["classification"]
    w = cl[0]["code"]
    layers = [{"shape": "rect", "width": W, "height": H, "x": W / 2, "y": H / 2, "fill": "radial-gradient(#1B1208, #05070B)"}]
    sq = 40
    band = [rect(i * sq, j * sq, sq, sq, "#FFFFFF") for i in range(0, 40) for j in range(3) if (i + j) % 2 == 0]
    layers.append(group(band, x=-200, y=110, anchor="topLeft", width=W, height=sq * 3, opacity=0.08, skewY=-6,
                        animate=[{"from": {"x": -20}, "to": {"x": -260}, "duration": dur, "ease": "linear"}]))
    layers.append(group([b for b in band], x=-200, y=H - 260, anchor="topLeft", width=W, height=sq * 3, opacity=0.08, skewY=-6,
                        animate=[{"from": {"x": -260}, "to": {"x": -20}, "duration": dur, "ease": "linear"}]))
    layers.append(text("WINNER", W / 2, 470, 34, RED, MONO, 800, "center", letterSpacing="0.4em", enter={"preset": "fadeIn", "delay": 0.3}))
    layers.append(text("KESSRA GRAND PRIX", W / 2, 520, 26, MUTED, MONO, 700, "center", letterSpacing="0.2em",
                       enter={"preset": "fadeIn", "delay": 0.4}))
    layers.append({"component": "text-3d", "x": W / 2, "y": 720, "width": 1000, "height": 320,
                   "props": {"text": NAME[w].split()[1].upper(), "gradient": ["#FFFFFF", COL[w], "#FFFFFF"],
                             "weight": 900, "tracking": 0.04, "depth": 0.22, "intro": "flyIn", "stagger": 0.06, "delay": 0.1,
                             "sway": 5, "shine": 1.2}})
    for k, row in enumerate(cl[:3]):
        y = 960 + k * 92
        c = row["code"]
        gap = "WINNER" if k == 0 else f"+{row['gap']:.3f}"
        layers.append(group([
            rect(M, y, W - 2 * M, 78, PANEL),
            rect(M, y, 78, 78, [YELLOW, "#C9D1DC", "#D08A4E"][k]),
            text(str(k + 1), M + 39, y + 39, 40, "#05070B", MONO, 900, "center"),
            rect(M + 100, y + 16, 6, 46, COL[c]),
            text(NAME[c].split()[1].upper(), M + 124, y + 30, 34, TEXT, HEAD, 800, "left"),
            text(TEAM[c], M + 126, y + 60, 18, MUTED, MONO, 700, "left"),
            text(gap, W - M - 24, y + 39, 32, TEXT, MONO, 800, "right"),
        ], enter={"preset": "fadeUp", "delay": 1.3 + 0.15 * k, "duration": 0.5, "ease": "outExpo"}))
    layers.append(text("58 LAPS  ·  296.496 KM", W / 2, 1290, 24, MUTED, MONO, 600, "center", letterSpacing="0.08em",
                       enter={"preset": "fadeIn", "delay": 2.0}))
    layers.append(text(f"FASTEST LAP {S.fmt_lap(DATA['fastest_lap']['time'])} ({DATA['fastest_lap']['code']})", W / 2, 1330, 24,
                       PURPLE, MONO, 700, "center", letterSpacing="0.08em", enter={"preset": "fadeIn", "delay": 2.1}))
    layers.append({"component": "confetti", "props": {"mode": "cannons", "at": 0.9, "count": 140, "colors": [COL[w], "#FFFFFF", RED], "power": 1.0}})
    return {"id": "winner", "duration": dur, "background": BG, "transition": {"type": "iris", "duration": 0.6}, "layers": layers}


def build():
    scenes = [overtake(), fastest(), winner()]
    # scene starts, as in spec.py
    starts, t = [], 0.0
    for i, sc in enumerate(scenes):
        tr = sc.get("transition")
        td = 0.0 if i == 0 or tr in (None, "cut") else (tr.get("duration", 0.5) if isinstance(tr, dict) else 0.5)
        t -= td
        starts.append(t)
        t += sc["duration"]
    st = dict(zip([sc["id"] for sc in scenes], starts))
    # The film's score, re-cut: replay section (its hit lands on the pass), fastest-lap lift, winner resolve.
    film = {"overtake": 20.45, "fastest": 28.2, "winner": 38.7}
    audio = []
    for k, sid in enumerate(("overtake", "fastest", "winner")):
        end = st[("fastest", "winner")[k]] if k < 2 else t
        audio.append({"src": "score.mp3", "at": r(st[sid]), "trimStart": film[sid], "duration": r(end - st[sid] + 0.15),
                      "volume": 0.67, "fadeIn": 0.03 if k else 0.0, "fadeOut": 0.15 if k < 2 else 1.0})
    audio += [
        {"sfx": "swoosh", "at": 0.25, "volume": 0.32},
        {"sfx": "impact", "at": r(st["overtake"] + S.OT_PASS), "volume": 0.39},
        {"sfx": "swoosh", "at": r(st["overtake"] + S.OT_PASS + 0.15), "volume": 0.28},
        {"sfx": "chime", "at": r(st["fastest"] + 0.1), "volume": 0.35},
        {"sfx": "success", "at": r(st["winner"] + 0.4), "volume": 0.3},
    ]
    spec = {
        "size": "9:16", "fps": S.FPS,
        "theme": {"colors": {"primary": RED, "accent": RED, "bg": BG, "surface": PANEL, "text": TEXT, "muted": MUTED},
                  "fonts": {"heading": "Inter Display", "body": "Inter", "mono": "JetBrains Mono"}},
        "background": BG,
        "scenes": scenes,
        "audio": audio,
    }
    out = os.path.join(S.HERE, "vertical.json")
    with open(out, "w") as f:
        json.dump(spec, f, separators=(",", ":"))
    print("wrote vertical.json", os.path.getsize(out) // 1024, "KB,", round(t, 2), "s")


if __name__ == "__main__":
    build()
