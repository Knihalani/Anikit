#!/usr/bin/env python3
"""Margin (fictional writing app that edits in the margin): 15 s, 16:9, 2D editorial typography, solo piano.

Idea: the film is one manuscript page. The camera reads down it; the product shows up as red pencil
in the margin. Motif: the vertical red margin rule, which becomes the logo's mark at the end.
Music: `piano` at 96 BPM (bar = 2.5 s); every move to the next passage lands on a downbeat.
"""
import json
BAR = 2.5
PAPER, INK, RED, GREY = "#F4EFE6", "#1B1A17", "#B8321C", "#6B655C"
SERIF, SANS, MONO = "Instrument Serif", "Inter", "JetBrains Mono"
MX = 1400            # x of the margin rule on the page
STEP = 1080          # page distance between passages

L = []               # layers of the page group (page coords, 1920 wide)
def text(s, x, y, size, color=INK, font=SERIF, start=0.0, **kw):
    d = {"text": s, "font": font, "size": size, "color": color, "x": x, "y": y, "anchor": "topLeft", "start": start}
    d.update(kw); L.append(d); return d
def strike(x, y, w, t, color=RED, thick=7):
    L.append({"shape": "path", "d": f"M0 6 C {w*0.3} 0, {w*0.7} 12, {w} 4", "stroke": color, "strokeWidth": thick, "fill": "none",
              "x": x, "y": y, "anchor": "topLeft", "start": t,
              "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "duration": 0.35, "ease": "inOutSine"}]})
def note(s, y, t, size=54):
    L.append({"text": f"*{s}*", "font": SERIF, "size": size, "color": RED, "x": MX + 60, "y": y, "anchor": "topLeft", "start": t,
              "animate": [{"from": {"opacity": 0, "x": MX + 90}, "duration": 0.35, "ease": "outCubic"}]})

# The margin rule: the motif, drawn down the whole page as the camera reads.
L.append({"shape": "rect", "width": 3, "height": 5 * STEP + 400, "fill": RED, "x": MX, "y": -400, "anchor": "topLeft", "opacity": 0.85})

# 1 (0–2.5 s): the problem, written plainly. Struck on the beat.
text("Your first draft", 200, 250, 150, letterSpacing="-0.01em", enter="mask by line up")
text("is a mess.", 200, 420, 150, letterSpacing="-0.01em", enter={"preset": "mask by line up", "delay": 0.25})
strike(384, 500, 276, 0.95)
note("honest.", 470, 1.15, 64)

# 2 (2.5–5 s): the product arrives as a reader in the margin.
y = STEP
text("Margin reads it", 200, y + 250, 150, start=0)
text("*with you.*", 200, y + 420, 150, RED, start=0)
for k, (w, t) in enumerate([("tighter", 3.0), ("cut this", 3.35), ("yes.", 3.7)]):
    note(w, y + 230 + k * 110, t, 58)

# 3 (5–7.5 s): it cuts the filler — shown on a real sentence.
y = 2 * STEP
text("CUTS THE FILLER", 200, y + 180, 50, GREY, SANS, letterSpacing="0.14em", weight=600)
text("This is a very, very", 200, y + 280, 96)
text("unique idea.", 200, y + 400, 96)
strike(600, y + 340, 190, 5.4)
note("one is plenty.", y + 300, 5.75, 54)

# 4 (7.5–10 s): it keeps your voice — the only mark is a quiet tick.
y = 3 * STEP
text("KEEPS YOUR VOICE", 200, y + 180, 50, GREY, SANS, letterSpacing="0.14em", weight=600)
text("“I wrote this at 3 a.m.", 200, y + 280, 96)
text("and I meant every word.”", 200, y + 400, 96)
note("leave it.", y + 330, 8.3, 58)

# 5 (10–15 s): the name, set on the margin rule.
y = 4 * STEP
text("Margin", MX - 60, y + 220, 340, INK, anchor="topRight", letterSpacing="-0.02em",
     enter={"preset": "mask by line up", "delay": 0.15})
text("Edits in the margin.", MX + 60, y + 330, 54, INK)
text("*Never in your voice.*", MX + 60, y + 400, 54, RED)
text("margin.app", MX + 60, y + 560, 50, GREY, MONO)

# Camera: the page slides up one passage per bar, landing on the downbeat.
moves = [{"to": {"y": -k * STEP + 200}, "at": k * BAR - 0.55, "duration": 0.55, "ease": "inOutCubic"} for k in range(1, 5)]
page = {"layers": L, "x": 0, "y": 200, "anchor": "topLeft", "width": 1920, "height": 5 * STEP + 400, "animate": moves}

spec = {"size": "16:9", "fps": 30, "background": PAPER, "intent": {"pace": "calm", "genre": "launch"},
        "music": {"style": "piano", "bpm": 96, "key": "F", "mood": "warm", "end": "ring"},
        "audio": [{"sfx": "tick", "at": 0.95, "volume": 0.35, "duck": 0}, {"sfx": "tick", "at": 5.4, "volume": 0.35, "duck": 0}, {"sfx": "chime", "at": 10.0, "volume": 0.3, "duck": 0}],
        "scenes": [{"duration": 15, "camera": {"y": "=t < 10 ? -40 + 80*fract((t + 0.55)/2.5) : -22.4 + 10*(t - 10)",
                                              "zoom": "=t < 10 ? 1 + 0.015*fract((t + 0.55)/2.5) : 1.0033 + 0.004*(t - 10)"}, "layers": [page]}]}
json.dump(spec, open(__file__.replace("make.py", "margin.json"), "w"), indent=1)
