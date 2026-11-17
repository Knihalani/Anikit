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
TOTAL, FRAMES = 55.0, 3300
# End-to-end wall time of `anikit-video render spec.json` (see README); update after re-measuring.
RENDER_SECONDS, RENDER_DECIMALS = 31, 0  # placeholder until the final measured render

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


grain = comp("grain", {"amount": 0.035, "rate": 0, "seed": 7})  # static: animated grain changes every pixel

scenes = []

# ================================================================ 1. hook
# 0 – 4 s. Two hard beats of type: "This film" / "is a JSON file."
scenes.append({
    "id": "hook", "duration": 4.5, "background": INK,
    "layers": [
        {"shape": "circle", "width": 1100, "height": 520, "fill": ICE, "opacity": 0, "blur": 160, "y": "52%",
         "animate": [{"to": {"opacity": 0.10}, "duration": 1.6, "ease": "outCubic"},
                     {"to": {"opacity": 0.22, "fill": LILAC}, "at": 2.0, "duration": 1.2}]},
        comp("dots", {"spacing": 44, "radius": 1.4, "color": "#ffffff14", "fade": 0.9}),
        text("This film", style="title", size="12vh", letterSpacing="-0.035em",
             kinetic="mask by word up 0.7s 90ms outExpo", end=1.9,
             exit={"preset": "blurOut", "duration": 0.25}),
        text("is a [JSON file.]{%s}" % ICE, style="title", size="11vh", letterSpacing="-0.035em",
             start=2.0, kinetic="mask by word up 0.7s 90ms outExpo"),
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
SPEC_DUR = 11.0  # cut on the beat at 15.5 s

# stage: the "screen" on the right that the spec compiles into
SW, SH, SX, SY = 800, 450, 1475, 540
ARCS = [[37.8, -122.4, 51.5, -0.1], [51.5, -0.1, 35.7, 139.7], [37.8, -122.4, -33.9, 151.2],
        [35.7, 139.7, 1.35, 103.8]]


def screen_layers(t_bg=None, t_aurora=None, t_title=None, t_globe=None, globe_age=0.0):
    """The 'Spec to Screen' frame, full size. Used full-frame in scene 3 and,
    scaled down, as the live stage in scene 2 (so the push-in matches exactly)."""
    def at(t, enter):
        return {} if t is None else {"start": t, "enter": enter}
    return [
        rect(1920, 1080, fill="#0b0d14", **({} if t_bg is None else
             {"opacity": 0, "animate": [{"to": {"opacity": 1}, "at": t_bg, "duration": 0.4}]})),
        comp("aurora", {"colors": [ICE, LILAC, MINT], "intensity": 0.75, "seed": 3, "rate": 4, "speed": 0.6},
             **at(t_aurora, {"preset": "fadeIn", "duration": 0.8})),
        comp("globe", {"arcs": ARCS, "markers": [[37.8, -122.4], [51.5, -0.1], [35.7, 139.7]], "scale": 1.0, "spin": 2.5,
                       "stagger": 0.25, "draw": 1.0}, height=1700, y="96%",
             **(at(t_globe, {"preset": "fadeUp", "duration": 0.7}) if t_globe is not None else {"start": -globe_age})),
        text("Spec to Screen", style="title", size="12vh", letterSpacing="-0.035em", y="36%",
             shadow={"x": 0, "y": 8, "blur": 40, "color": "#0009"},
             **({} if t_title is None else {"start": t_title, "kinetic": "rise by char 0.6s 25ms outExpo"})),
    ]


ZOOM = 1920 / SW
stage_layers = screen_layers(t_bg=done(4), t_aurora=done(6), t_title=done(7), t_globe=done(8))
scenes.append({
    "id": "spec", "duration": SPEC_DUR, "background": INK,
    "transition": {"type": "cut"},
    "camera": {"animate": [
        *[{"to": {"zoom": round(1.0 + 0.0035 * (k + 1), 4)}, "at": done(i), "duration": 0.4, "ease": "outCubic"}
          for k, i in enumerate((1, 2, 4, 6, 7, 8))],
        {"to": {"zoom": ZOOM, "x": SX - 960, "y": SY - 540}, "at": SPEC_DUR - 1.1, "duration": 1.1, "ease": "inOutQuart"},
    ]},
    "layers": [
        comp("grid", {"cell": 64, "color": "#ffffff0a", "fade": 0.8}),
                comp("code", {"code": CODE, "language": "json", "title": "spec.json", "fontSize": 32, "typing": CPS,
                      "delay": TYPE_AT, "chrome": "mac", "glow": "#7cc4ff30",
                      "highlights": [{"lines": [i + 1], "at": done(i) - TYPE_AT, "until": done(i) - TYPE_AT + 0.7, "dim": False}
                                     for i in (1, 2, 4, 6, 7, 8)]},
             x=555, y=SY, width=1010, height=820, enter={"preset": "fadeUp", "duration": 0.6}),
        # the stage frame draws on when "size" is typed
        rect(SW + 2, SH + 2, radius=19, fill="none", stroke={"color": "#ffffff33", "width": 2}, x=SX, y=SY,
             start=done(1) - 0.6, enter={"preset": "drawOn", "duration": 0.6}),
        text("1920 × 1080 · [60 fps]{%s}" % ICE, font="$mono", size=26, color=MUTED, x=SX - SW / 2,
             y=SY + SH / 2 + 34, anchor="left", start=done(2), enter="fadeIn"),
        {"type": "group", "x": SX, "y": SY, "width": 1920, "height": 1080, "scale": SW / 1920, "clip": True,
         "radius": 40, "layers": stage_layers},
        grain,
    ],
})

# ================================================================ 3. the screen it described
scenes.append({
    "id": "screen", "duration": 4.0, "background": "#0b0d14",
    "transition": {"type": "cut"},
    "layers": [
        *screen_layers(globe_age=round(SPEC_DUR - done(8), 3)),
        grain,
    ],
})

# ================================================================ 4. kinetic: write / check / render
words = [("Write it.", "spec.json"), ("Check it.", "anikit-video check spec.json"),
         ("Render it.", "anikit-video render spec.json")]
kin_layers = [comp("dots", {"spacing": 44, "radius": 1.4, "color": "#ffffff12", "fade": 0.9})]
for i, (w, cmd) in enumerate(words):
    s, e = i * 2.0, i * 2.0 + 2.0
    kin_layers += [
        text(w, style="title", size="16vh", letterSpacing="-0.04em", start=s, end=e, y="46%",
             kinetic="mask by char up 0.45s 22ms outExpo",
             color=[TEXT, TEXT, ICE][i]),
        text(cmd, font="$mono", size="3vh", color=MUTED, start=s, end=e, y="62%",
             enter={"preset": "fadeIn", "duration": 0.2}),
    ]
kin_layers += [
    text("No browser.", style="title", size="11vh", letterSpacing="-0.035em", start=6.0, y="40%",
         kinetic="blur by word 0.6s 80ms outCubic", color=TEXT),
    text("No screen capture.", style="title", size="11vh", letterSpacing="-0.035em", start=6.5, y="54%",
         kinetic="blur by word 0.6s 80ms outCubic", color=MUTED),
    grain,
]
scenes.append({"id": "kinetic", "duration": 8.5, "background": INK, "transition": {"type": "cut"},
               "layers": kin_layers})

# ================================================================ 5. devices
scenes.append({
    "id": "devices", "duration": 7.5, "background": "radial-gradient(#141826, %s)" % INK,
    "transition": {"type": "push", "direction": "up", "duration": 0.5, "ease": "inOutQuart"},
    "layers": [
        comp("bokeh", {"count": 14, "colors": [ICE, LILAC], "minSize": 60, "maxSize": 220, "speed": 0.3, "seed": 5},
             opacity=0.35),
        text("Product shots, [generated.]{%s}" % LILAC, style="heading", size="6.4vh", letterSpacing="-0.025em",
             y="12%", kinetic="rise by word 0.6s 70ms outExpo"),
        comp("laptop", {"screen": "dashboard", "title": "spec.json", "accent": ICE,
                         "items": ["hook", "spec", "screen", "kinetic", "devices", "speed", "chart", "particles", "logo"],
                         "kpis": [{"label": "Frames", "value": format(FRAMES, ",")}, {"label": "Scenes", "value": "9"},
                                  {"label": "Seconds", "value": "%g" % TOTAL}], "open": 1.0, "delay": 0.3,
                        "shadow": True}, width=1180, height=720, x="44%", y="58%"),
        comp("phone", {"screen": "chat", "title": "anikit-video", "accent": LILAC,
                       "items": ["JSON in", "MP4 out", "No browser in the loop"], "delay": 1.1, "float": 6,
                       "time": "9:41"}, width=360, height=700, x="77%", y="60%"),
        comp("cursor", {"path": [[1150, 980], [640, 540], [905, 540], [1150, 620], [1500, 640]],
                        "times": [1.6, 2.6, 3.6, 4.6, 5.8], "clicks": [2.7, 3.7, 4.7, 5.9], "size": 34},
             start=0),
        comp("badge", {"label": '"screen": "dashboard"', "color": ICE, "delay": 2.2}, width=420, height=56,
             x="30%", y="93%"),
        comp("badge", {"label": '"screen": "chat"', "color": LILAC, "delay": 2.5}, width=320, height=56,
             x="77%", y="93%"),
        grain,
    ],
})

# ================================================================ 6. numbers (measured; see README)
scenes.append({
    "id": "speed", "duration": 4.0, "background": INK, "transition": {"type": "cut"},
    "layers": [
        comp("grid", {"cell": 80, "color": "#ffffff08", "fade": 0.85}),
        {"shape": "circle", "width": 1200, "height": 600, "fill": ICE, "opacity": 0.10, "blur": 140, "y": "46%"},
        text("This %d-second film rendered in" % round(TOTAL), style="heading", size="5.2vh", y="22%",
             letterSpacing="-0.02em", kinetic="rise by word 0.5s 60ms outExpo"),
        comp("counter", {"from": 0, "to": RENDER_SECONDS, "decimals": RENDER_DECIMALS, "suffix": " s", "size": 300,
                         "duration": 2.2, "delay": 0.4, "gradient": [TEXT, ICE]},
             width=1400, height=380, y="47%"),
        comp("progress-bar", {"from": 0, "to": 100, "duration": 2.2, "delay": 0.4, "thickness": 8, "color": ICE, "percent": False},
             width=900, height=40, y="66%"),
        text("%s frames · 1080p60 · one 4-core VM" % format(FRAMES, ","), style="caption",
             size="2.6vh", y="71%", enter={"preset": "fadeIn", "delay": 0.6}),
        grain,
    ],
})
scenes.append({
    "id": "chart", "duration": 5.0, "background": INK, "transition": {"type": "cut"},
    "layers": [
        text("60 s of 1080p60 motion graphics → MP4", style="heading", size="5vh", x=160, y="15%",
             anchor="left", kinetic="rise by word 0.5s 50ms outExpo"),
        text("Wall time in seconds, same composition, default settings · lower is better", style="caption", x=160,
             y="22%", anchor="left", enter={"preset": "fadeIn", "delay": 0.3}),
        comp("bar-chart", {"values": [6.95, 139.5, 152.0], "labels": ["Anikit Video", "Remotion", "HyperFrames"],
                           "horizontal": True, "highlight": 0, "colors": [ICE, MUTED, MUTED], "fontSize": 30,
                           "format": {"suffix": " s", "decimals": 1}, "delay": 0.4, "stagger": 0.15, "grid": False},
             width=1000, height=440, x=160 + 500, y="56%"),
        comp("counter", {"from": 1, "to": 20, "suffix": "×", "size": 220, "delay": 1.2,
                         "duration": 1.4, "gradient": [TEXT, ICE], "label": "faster than Remotion"},
             width=480, height=420, x=1500, y="54%"),
        text("Measured: docs/video/BENCHMARKS.md · 4 vCPU Xeon · median of 3", font="$mono", size=24, color=MUTED,
             x=160, y="86%", anchor="left", enter={"preset": "fadeIn", "delay": 1.0}),
        grain,
    ],
})

# ================================================================ 7. particles
scenes.append({
    "id": "particles", "duration": 4.0, "background": INK, "transition": {"type": "crossfade", "duration": 0.5},
    "layers": [
        comp("particleText", {"words": ["JSON in.", "Film out."], "count": 2600, "size": 4.2, "glow": False, "colors": [ICE, LILAC],
                              "form": 0.8, "hold": 0.8, "exit": "explode", "exitTime": 0.8, "seed": 4}),
    ],
})

# ================================================================ 8. logo
LOGO = 8.0
scenes.append({
    "id": "logo", "duration": LOGO, "background": "radial-gradient(#10131d, #040508)",
    "transition": {"type": "fadeBlack", "duration": 0.5},
    "layers": [
        comp("godRays", {"at": [0.5, -0.3], "angle": 90, "spread": 60, "color": "#cfe3ff", "intensity": 0.35,
                         "dust": 30, "spin": 0}, width=1920, height=640, y=320),
        comp("text-3d", {"text": "ANIKIT", "gradient": ["#dfe9ff", "#e9e0ff"], "intro": "rise", "stagger": 0.08,
                        "delay": 0.2, "tilt": 10, "sway": 0, "shine": 0, "scale": 0.78}, y="45%"),
        comp("lensFlare", {"path": [[0.12, 0.36], [0.88, 0.40]], "times": [1.6, 3.6], "intensity": 0.8, "ghosts": 3},
             start=0, end=5.4, exit={"preset": "fadeOut", "duration": 0.8}, blend="screen"),
        text("VIDEO", style="label", size="3.2vh", letterSpacing="0.6em", color=TEXT, y="67%",
             enter={"preset": "fadeIn", "delay": 1.6, "duration": 0.8}),
        text("anikit-video render spec.json", font="$mono", size="2.4vh", color=MUTED, y="80%",
             enter={"preset": "fadeUp", "delay": 2.8}),
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
    {"src": "music.mp3", "volume": 0.9},  # python3 music.py (120 BPM; cuts land on beats)
    {"sfx": "typing", "at": starts["spec"] + TYPE_AT, "duration": sched[-1][1], "volume": 0.3},
    {"sfx": "whoosh", "at": starts["screen"] - 0.35, "volume": 0.3},
    {"sfx": "swoosh", "at": starts["devices"] - 0.2, "volume": 0.3},
    {"sfx": "swoosh", "at": starts["chart"] - 0.2, "volume": 0.25},
    {"sfx": "whoosh", "at": starts["particles"] + 3.4, "volume": 0.3},
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
