#!/usr/bin/env python3
"""Generates spec.json for "Spec to Screen", the Anikit Video launch film.

The film is plain anikit-video JSON; this script only exists so the stage
events in scene 2 land exactly when the code block finishes typing each line
(it mirrors the code component's deterministic typing rhythm).

    python3 examples/video/spec-to-screen/build.py
"""
import json, os

HERE = os.path.dirname(os.path.abspath(__file__))
BEAT = 0.5  # 120 BPM grid: every cut lands on a beat

# ---------------------------------------------------------------- palette
INK, SURF, TEXT, MUTED = "#07080c", "#12141c", "#f4f2ee", "#8b90a0"
ICE, LILAC, MINT = "#7cc4ff", "#b9a6ff", "#7af0c8"


def hash01(x):
    """Same integer hash as components/style.rs `hash01`."""
    m = 0xFFFFFFFF
    x ^= x >> 16
    x = (x * 0x7FEB352D) & m
    x ^= x >> 15
    x = (x * 0x846CA68B) & m
    x ^= x >> 16
    return (x >> 8) / float(1 << 24)


def typing_schedule(code, cps):
    """(start, end) of each line, as the `code` component types it."""
    base, t, n, out = 1.0 / cps, 0.0, 0, []
    for line in code.split("\n"):
        start = t
        for c in line.lstrip():
            n += 1
            t += base * (0.6 + 0.8 * hash01(n)) * (1.8 if c in ",;{(" else 1.0)
        out.append((start, t))
        t += base * 4.0
    return out


def text(s, **kw):
    d = {"text": s}
    d.update(kw)
    return d


def comp(kind, props=None, **kw):
    d = {"component": kind, "props": props or {}}
    d.update(kw)
    return d


def rect(w, h, **kw):
    d = {"shape": "rect", "width": w, "height": h}
    d.update(kw)
    return d


grain = comp("grain", {"amount": 0.035, "rate": 24, "seed": 7})

scenes = []

# ================================================================ 1. hook
# 0 – 4 s. Two hard beats of type: "This film" / "is a JSON file."
scenes.append({
    "id": "hook", "duration": 4.5, "background": INK,
    "layers": [
        comp("dots", {"spacing": 44, "radius": 1.4, "color": "#ffffff14", "fade": 0.9}),
        text("This film", style="title", size="12vh", letterSpacing="-0.035em",
             kinetic="mask by word up 0.7s 90ms outExpo", end=1.9,
             exit={"preset": "blurOut", "duration": 0.25}),
        text("is a [JSON file.]{%s}" % ICE, style="title", size="11vh", letterSpacing="-0.035em",
             start=2.0, kinetic="mask by word up 0.7s 90ms outExpo"),
        text("{ }", font="$mono", size="3vh", color=MUTED, y="74%", start=2.6, enter="fadeUp"),
        grain,
    ],
})

# ================================================================ 2. spec types itself
CODE = """{
  "size": "16:9",
  "fps": 60,
  "scenes": [{
    "background": "#0b0d14",
    "layers": [
      { "component": "aurora" },
      { "text": "Spec to Screen" },
      { "component": "globe" }
    ]
  }]
}"""
CPS, TYPE_AT = 26.0, 0.6
sched = typing_schedule(CODE, CPS)
done = lambda i: round(TYPE_AT + sched[i][1] + 0.05, 2)  # line i finished typing
SPEC_DUR = round(done(11) + 2.6, 1)

# stage: the "screen" on the right that the spec compiles into
SW, SH, SX, SY = 860, 484, 1390, 560
stage_layers = [
    rect(SW, SH, radius=18, fill="#0b0d14", opacity=0, x="50%", y="50%",
         animate=[{"to": {"opacity": 1}, "at": done(4), "duration": 0.4}]),
    comp("aurora", {"colors": [ICE, LILAC, MINT], "intensity": 0.7, "seed": 3},
         start=done(6), enter={"preset": "fadeIn", "duration": 0.8}),
    comp("globe", {"arcs": [[37.8, -122.4, 51.5, -0.1], [51.5, -0.1, 35.7, 139.7], [37.8, -122.4, -33.9, 151.2]],
                   "markers": [[37.8, -122.4], [51.5, -0.1]], "scale": 0.9, "stagger": 0.3, "draw": 1.0},
         width=SW, height=SH * 1.6, y="88%", start=done(8), enter={"preset": "fadeUp", "duration": 0.7}),
    text("Spec to Screen", style="title", size=64, letterSpacing="-0.03em", y="40%",
         start=done(7), kinetic="rise by char 0.6s 25ms outExpo"),
]
scenes.append({
    "id": "spec", "duration": SPEC_DUR, "background": INK,
    "transition": {"type": "cut"},
    "camera": {"animate": [
        {"from": {"zoom": 1.0}, "to": {"zoom": 1.03}, "at": 0, "duration": SPEC_DUR - 1.4, "ease": "linear"},
        {"to": {"zoom": 2.15, "x": SX - 960, "y": SY - 540}, "at": SPEC_DUR - 1.4, "duration": 1.4, "ease": "inOutQuart"},
    ]},
    "layers": [
        comp("grid", {"cell": 64, "color": "#ffffff0a", "fade": 0.8}),
                comp("code", {"code": CODE, "language": "json", "title": "spec.json", "fontSize": 25, "typing": CPS,
                      "delay": TYPE_AT, "chrome": "mac", "glow": "#7cc4ff30",
                      "highlights": [{"lines": [i + 1], "at": done(i) - TYPE_AT, "until": done(i) - TYPE_AT + 0.7}
                                     for i in (1, 2, 4, 6, 7, 8)]},
             x=520, y=SY, width=800, height=520, enter={"preset": "fadeUp", "duration": 0.6}),
        # the stage frame draws on when "size" is typed
        rect(SW + 2, SH + 2, radius=19, fill="none", stroke={"color": "#ffffff33", "width": 2}, x=SX, y=SY,
             start=done(1) - 0.6, enter={"preset": "drawOn", "duration": 0.6}),
        text("1920 × 1080", font="$mono", size=24, color=MUTED, x=SX - SW / 2, y=SY - SH / 2 - 26, anchor="left",
             start=done(1), enter="fadeIn"),
        text("60 fps", font="$mono", size=24, color=ICE, x=SX + SW / 2, y=SY - SH / 2 - 26, anchor="right",
             start=done(2), enter="popIn"),
        {"type": "group", "x": SX, "y": SY, "width": SW, "height": SH, "clip": True, "radius": 18,
         "layers": stage_layers},
        grain,
    ],
})

# ================================================================ 3. the screen it described
scenes.append({
    "id": "screen", "duration": 4.0, "background": "#0b0d14",
    "transition": {"type": "crossfade", "duration": 0.3},
    "camera": {"animate": [{"from": {"zoom": 1.06}, "to": {"zoom": 1.0}, "duration": 4, "ease": "outCubic"}]},
    "layers": [
        comp("aurora", {"colors": [ICE, LILAC, MINT], "intensity": 0.75, "seed": 3}),
        comp("globe", {"arcs": [[37.8, -122.4, 51.5, -0.1], [51.5, -0.1, 35.7, 139.7], [37.8, -122.4, -33.9, 151.2],
                                [35.7, 139.7, 1.35, 103.8]],
                       "markers": [[37.8, -122.4], [51.5, -0.1], [35.7, 139.7]], "scale": 1.0, "stagger": 0.25,
                       "draw": 1.0}, height=1700, y="96%"),
        text("Spec to Screen", style="title", size="12vh", letterSpacing="-0.035em", y="36%"),
        text("one file  →  one film", font="$mono", size="2.6vh", color=MUTED, y="50%",
             enter={"preset": "fadeUp", "delay": 0.6}),
        grain,
    ],
})

# ================================================================ 4. kinetic: write / check / render
words = [("Write it.", "spec.json"), ("Check it.", "anikit-video check spec.json"),
         ("Render it.", "anikit-video render spec.json -o film.mp4")]
kin_layers = [comp("dots", {"spacing": 44, "radius": 1.4, "color": "#ffffff12", "fade": 0.9})]
for i, (w, cmd) in enumerate(words):
    s, e = i * 1.25, i * 1.25 + 1.25
    kin_layers += [
        text(w, style="title", size="16vh", letterSpacing="-0.04em", start=s, end=e, y="46%",
             kinetic="mask by char up 0.45s 22ms outExpo",
             color=[TEXT, TEXT, ICE][i]),
        text(cmd, font="$mono", size="2.6vh", color=MUTED, start=s + 0.15, end=e, y="62%",
             enter={"preset": "fadeIn", "duration": 0.2}),
    ]
kin_layers += [
    text("No browser.", style="title", size="11vh", letterSpacing="-0.035em", start=3.75, y="40%",
         kinetic="blur by word 0.6s 80ms outCubic", color=TEXT),
    text("No screen capture.", style="title", size="11vh", letterSpacing="-0.035em", start=4.25, y="54%",
         kinetic="blur by word 0.6s 80ms outCubic", color=MUTED),
    text("Native Rust, straight to H.264.", style="subtitle", start=5.0, y="70%",
         enter="fadeUp"),
    grain,
]
scenes.append({"id": "kinetic", "duration": 8.0, "background": INK, "transition": {"type": "cut"},
               "layers": kin_layers})

# ================================================================ 5. devices
scenes.append({
    "id": "devices", "duration": 7.5, "background": "radial-gradient(#141826, %s)" % INK,
    "transition": {"type": "push", "direction": "up", "duration": 0.5, "ease": "inOutQuart"},
    "camera": {"animate": [{"from": {"zoom": 1.0}, "to": {"zoom": 1.06, "x": 40}, "duration": 7.5, "ease": "inOutSine"}]},
    "layers": [
        comp("bokeh", {"count": 14, "colors": [ICE, LILAC], "minSize": 60, "maxSize": 220, "speed": 0.3, "seed": 5},
             opacity=0.35),
        text("Product shots, [generated.]{%s}" % LILAC, style="heading", size="6.4vh", letterSpacing="-0.025em",
             y="12%", kinetic="rise by word 0.6s 70ms outExpo"),
        comp("laptop", {"screen": "dashboard", "title": "Northwind", "accent": ICE, "open": 1.0, "delay": 0.3,
                        "shadow": True}, width=1180, height=720, x="44%", y="58%"),
        comp("phone", {"screen": "chat", "title": "Relay", "accent": LILAC, "delay": 1.1, "float": 6,
                       "time": "9:41"}, width=360, height=700, x="77%", y="60%"),
        comp("toast", {"title": "film.mp4 ready", "body": "1080p · 60 fps", "kind": "success", "delay": 3.6,
                       "hold": 2.4, "from": "right", "width": 420}, width=1920, height=1080),
        comp("badge", {"label": '"screen": "dashboard"', "color": ICE, "delay": 2.2}, width=420, height=56,
             x="30%", y="93%"),
        comp("badge", {"label": '"screen": "chat"', "color": LILAC, "delay": 2.5}, width=320, height=56,
             x="77%", y="93%"),
        grain,
    ],
})

# ================================================================ 6. numbers (measured; see README)
scenes.append({
    "id": "speed", "duration": 5.0, "background": INK, "transition": {"type": "cut"},
    "layers": [
        comp("grid", {"cell": 80, "color": "#ffffff08", "fade": 0.85}),
        text("MEASURED · 1080p · 4 CPU CORES", style="label", size=26, y="17%", enter="fadeIn"),
        comp("counter", {"to": 935, "suffix": " fps", "size": 190, "label": "frames rendered per second", "duration": 1.6,
                         "gradient": [TEXT, ICE]}, width=860, height=420, x="28%", y="50%"),
        comp("counter", {"to": 1268, "suffix": " fps", "size": 190, "label": "frames encoded per second", "duration": 1.6,
                         "delay": 0.4, "gradient": [TEXT, LILAC]}, width=860, height=420, x="72%", y="50%"),
        text("ak264: our own H.264 encoder.", style="subtitle", y="80%",
             enter={"preset": "fadeUp", "delay": 1.6}),
        grain,
    ],
})
scenes.append({
    "id": "chart", "duration": 6.0, "background": INK, "transition": {"type": "slide", "direction": "left", "duration": 0.5, "ease": "inOutQuart"},
    "layers": [
        text("Encode speed, typical motion graphics", style="heading", size="5vh", x=160, y="15%", anchor="left",
             kinetic="rise by word 0.5s 50ms outExpo"),
        text("1080p · 240 frames · same quality (≈ 50 dB PSNR) · frames per second", style="caption", x=160,
             y="22%", anchor="left", enter={"preset": "fadeIn", "delay": 0.3}),
        comp("bar-chart", {"values": [147, 1268, 1389], "labels": ["x264 veryfast", "ak264", "ak264 + renderer hints"],
                           "horizontal": True, "highlight": 2, "colors": [MUTED, ICE, LILAC], "fontSize": 30,
                           "delay": 0.4, "stagger": 0.15, "grid": False},
             width=1120, height=520, x=160 + 560, y="58%"),
        comp("counter", {"from": 1, "to": 9.4, "decimals": 1, "suffix": "×", "size": 220, "delay": 1.2,
                         "duration": 1.4, "gradient": [TEXT, LILAC], "label": "faster than x264 veryfast"},
             width=560, height=420, x="83%", y="56%"),
        text("Source: docs/video/encoder.md", font="$mono", size=24, color=MUTED, x=160, y="89%", anchor="left",
             enter={"preset": "fadeIn", "delay": 1.0}),
        grain,
    ],
})

# ================================================================ 7. particles
scenes.append({
    "id": "particles", "duration": 5.0, "background": INK, "transition": {"type": "crossfade", "duration": 0.4},
    "layers": [
        comp("particleText", {"words": ["JSON in.", "Film out."], "count": 9000, "size": 2.8, "colors": [ICE, LILAC],
                              "form": 1.1, "hold": 0.9, "exit": "explode", "exitTime": 1.0, "seed": 4}),
    ],
})

# ================================================================ 8. logo
LOGO = 8.0
scenes.append({
    "id": "logo", "duration": LOGO, "background": "radial-gradient(#10131d, #040508)",
    "transition": {"type": "fadeWhite", "duration": 0.35},
    "camera": {"animate": [{"from": {"zoom": 1.08}, "to": {"zoom": 1.0}, "duration": LOGO, "ease": "outCubic"}]},
    "layers": [
        comp("godRays", {"at": [0.5, -0.15], "angle": 90, "spread": 60, "color": "#cfe3ff", "intensity": 0.35,
                         "dust": 80}),
        comp("text-3d", {"text": "ANIKIT", "gradient": ["#dfe9ff", "#e9e0ff"], "intro": "rise", "stagger": 0.08,
                        "delay": 0.2, "tilt": 10, "sway": 4, "scale": 0.78}, y="45%"),
        comp("lensFlare", {"path": [[0.12, 0.36], [0.88, 0.40]], "times": [1.6, 3.6], "intensity": 0.9},
             start=0, blend="screen"),
        text("VIDEO", style="label", size="3.2vh", letterSpacing="0.6em", color=TEXT, y="64%",
             enter={"preset": "fadeIn", "delay": 1.6, "duration": 0.8}),
        text("anikit-video render spec.json", font="$mono", size="2.4vh", color=MUTED, y="80%",
             enter={"preset": "fadeUp", "delay": 2.8}),
        grain,
    ],
})

# ================================================================ audio
starts, t = {}, 0.0
for k, s in enumerate(scenes):
    tr = s.get("transition", {"type": "crossfade", "duration": 0.4})
    t -= tr.get("duration", 0.5) if k and tr.get("type") != "cut" else 0.0
    starts[s["id"]] = round(t, 3)
    t += s["duration"]

audio = [
    {"sfx": "whoosh", "at": 1.95, "volume": 0.35},
    {"sfx": "typing", "at": starts["spec"] + TYPE_AT, "duration": sched[-1][1], "volume": 0.4},
    {"sfx": "riser", "at": starts["spec"] + SPEC_DUR - 1.6, "duration": 1.6, "volume": 0.5},
    {"sfx": "impact", "at": starts["screen"] + 0.1, "volume": 0.6},
    {"sfx": "tick", "at": starts["kinetic"], "volume": 0.6},
    {"sfx": "tick", "at": starts["kinetic"] + 1, "volume": 0.6},
    {"sfx": "tick", "at": starts["kinetic"] + 2, "volume": 0.6},
    {"sfx": "swoosh", "at": starts["devices"], "volume": 0.4},
    {"sfx": "ding", "at": starts["speed"] + 1.7, "volume": 0.35},
    {"sfx": "swoosh", "at": starts["chart"], "volume": 0.35},
    {"sfx": "whoosh", "at": starts["particles"] + 3.4, "volume": 0.4},
    {"sfx": "impact", "at": starts["logo"] + 0.3, "volume": 0.6},
    {"sfx": "chime", "at": starts["logo"] + 1.7, "volume": 0.4},
]

spec = {
    "size": "16:9", "fps": 60,
    "theme": {
        "colors": {"primary": ICE, "secondary": MINT, "accent": LILAC, "bg": INK, "surface": SURF, "text": TEXT,
                   "muted": MUTED},
        "fonts": {"heading": "Inter Display", "body": "Inter", "mono": "JetBrains Mono"},
    },
    "transition": {"type": "crossfade", "duration": 0.4},
    "scenes": scenes,
    "audio": audio,
}

with open(os.path.join(HERE, "spec.json"), "w") as f:
    json.dump(spec, f, indent=1, ensure_ascii=False)
    f.write("\n")
print("scene starts:", starts, "total ≈", round(t, 2), "s")
print("typing line ends:", [done(i) for i in range(len(sched))])
