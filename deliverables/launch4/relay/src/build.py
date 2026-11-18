#!/usr/bin/env python3
"""Builds the Relay launch film spec (16:9 hero and 9:16 cut).

The film is one night on one clock. The ground's colour is computed from the
hour; inbox rows arrive as the clock advances; Relay reads them (amber mark +
tag); at 07:00 the night folds into one brief and the viewer sends drafts.

usage: python3 src/build.py            -> spec.json (16:9) and spec-9x16.json
"""
import json, os, sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.dirname(HERE)

FPS = 30
BAR = 2.0  # 120 bpm, 4/4

# palette
INK, INK_CARD, INK_LINE = "#0E1A2E", "#17253B", "#FFFFFF14"
N_TEXT, N_SUB, N_MUTE = "#EEF2F8", "#C9D3E1", "#8193AD"
DAY_CARD, D_TEXT, D_SUB, D_MUTE = "#FBFAF7", "#14181F", "#2E3642", "#5B6573"
AMBER = "#FFB23E"
UI = "Host Grotesk"
DISP = "Bricolage Grotesque"

GROUND = """uniform float hour; // = 0.0
void mainImage(out vec4 o, in vec2 f){
  vec2 uv = f / iResolution.xy;            // y = 0 at the bottom
  float k = clamp(hour, 0.0, 1.0);         // 0 = night, 1 = 07:00 daylight
  float k2 = k*k*(3.0-2.0*k);
  vec3 nTop = vec3(0.047, 0.086, 0.157), nBot = vec3(0.085, 0.140, 0.235);
  vec3 dTop = vec3(0.780, 0.835, 0.885), dBot = vec3(0.995, 0.900, 0.745);
  float y = pow(1.0 - uv.y, 1.5);
  vec3 c = mix(mix(nTop, nBot, y), mix(dTop, dBot, y), k2);
  // first light: a warm band at the horizon that rises then settles
  float band = exp(-7.0 * uv.y) * sin(3.14159 * clamp(k * 1.15, 0.0, 1.0));
  c += band * vec3(0.55, 0.30, 0.10) * 0.55;
  // gentle falloff toward the corners (one light: the sky)
  vec2 d = uv - vec2(0.5, 0.15);
  c *= 1.0 - 0.18 * dot(d, d);
  o = vec4(c, 1.0);
}"""


def ground(h0, h1=None, dur=None, ease="inOutCubic", at=0):
    L = {"shader": GROUND, "uniforms": {"hour": h0}, "width": "130%", "height": "130%"}
    if h1 is not None:
        L["animate"] = [{"from": {"hour": h0}, "to": {"hour": h1}, "duration": dur, "ease": ease, "at": at}]
    return L


def fmt_clock(m):
    m = int(m) % (24 * 60)
    return "%02d:%02d" % (m // 60, m % 60)


def clock(keys, dur, size="62vh", x="50%", y="50%", color="#FFFFFF1C", anchor="center", extra=None, axes=None):
    """keys: [(t, minutes)] piecewise; eased (inOutCubic) between keys.
    Emits one text layer per distinct label run."""
    def mins(t):
        for (t0, m0), (t1, m1) in zip(keys, keys[1:]):
            if t0 <= t <= t1:
                p = (t - t0) / (t1 - t0) if t1 > t0 else 1
                p = 4 * p ** 3 if p < 0.5 else 1 - (-2 * p + 2) ** 3 / 2
                return m0 + (m1 - m0) * p
        return keys[-1][1] if t > keys[-1][0] else keys[0][1]
    runs = []
    n = int(round(dur * FPS))
    for f in range(n):
        t = f / FPS
        lab = fmt_clock(mins(t))
        if runs and runs[-1][0] == lab:
            runs[-1][2] = (f + 1) / FPS
        else:
            runs.append([lab, t, (f + 1) / FPS])
    out = []
    for lab, a, b in runs:
        L = {"text": lab, "font": DISP, "weight": 800, "size": size, "features": "tnum",
             "letterSpacing": "-0.045em", "color": color, "x": x, "y": y, "anchor": anchor,
             "start": round(a, 4), "end": round(b, 4) if b < dur - 1e-6 else dur}
        if axes:
            L["axes"] = axes
        if extra:
            L.update(extra)
        out.append(L)
    return out


def group(x, y, layers, **kw):
    g = {"x": x, "y": y, "anchor": "topLeft", "width": 1920, "height": 1080, "layers": layers}
    g.update(kw)
    return g


def text(s, x, y, size, color, weight=500, anchor="topLeft", font=UI, **kw):
    L = {"text": s, "font": font, "weight": weight, "size": size, "color": color, "x": x, "y": y, "anchor": anchor}
    L.update(kw)
    return L


def rect(x, y, w, h, fill, radius=0, anchor="topLeft", **kw):
    L = {"shape": "rect", "x": x, "y": y, "width": w, "height": h, "fill": fill, "radius": radius, "anchor": anchor}
    L.update(kw)
    return L


def row(x, y, sender, when, subject, snip, read=None, tag=None, tag_amber=True, w=1200, arrive=0.0,
        night=True, enter=True, extra=None):
    """One inbox row. `read`: time (layer-local to scene) when Relay reads it."""
    card, t1, t2, t3 = (INK_CARD, N_TEXT, N_SUB, N_MUTE) if night else (DAY_CARD, D_TEXT, D_SUB, D_MUTE)
    H = 168
    L = [
        rect(0, 0, w, H, card, 28, stroke=INK_LINE if night else "#14181F12", strokeWidth=2),
        {"shape": "circle", "x": 24, "y": 54, "width": 10, "height": 10, "fill": t1, "anchor": "center",
         **({"end": read - arrive} if read is not None else {})},
        text(sender, 44, 30, 38, t1, 650),
        text(when, w - 40, 34, 30, t3, 500, anchor="topRight", features="tnum"),
        text(subject, 44, 80, 32, t2, 550),
        text(snip, 44, 122, 28, t3, 400),
    ]
    if read is not None:
        r = read - arrive
        # the read mark: an amber hairline sweeps the bottom of the card
        L.append({"shape": "path", "d": "M0 0 L%d 0" % (w - 56), "stroke": AMBER, "strokeWidth": 3,
                  "x": 28, "y": H - 2, "anchor": "topLeft", "start": r,
                  "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "duration": 0.5, "ease": "outExpo"}]})
        if tag:
            L.append(text(tag, w - 40, 84, 28, AMBER if tag_amber else t3, 650, anchor="topRight",
                          start=r + 0.2, enter={"preset": "fadeUp", "duration": 0.25, "distance": 10}))
    g = group(x, y, L, start=arrive)
    if enter:
        g["enter"] = {"preset": "fadeUp", "duration": 0.5, "distance": 40, "ease": "outExpo"}
    if extra:
        g.update(extra)
    return g


def claim(s, x, y, size, color, start=0, end=None, anchor="topLeft", enter="mask by line up", **kw):
    L = {"text": s, "font": DISP, "weight": 800, "size": size, "color": color, "x": x, "y": y, "anchor": anchor,
         "letterSpacing": "-0.03em", "lineHeight": 0.92, "start": start, "enter": enter}
    if end is not None:
        L["end"] = end
    L.update(kw)
    return L


# --------------------------------------------------------------------------------------------
# the night's mail (illustrative content; every string is plausible inbox material)
NIGHT = [
    # sender, time, subject, snippet, tag
    ("Dana Whitfield", "11:58 PM", "One small thing before tomorrow", "Can you send the Q3 numbers to the board before 9?", "Draft ready"),
    ("Priya Shah", "12:41 AM", "Design review", "Does Thursday work? Happy to move if not.", "Draft ready"),
    ("Stripe", "1:07 AM", "Invoice INV-2291 paid", "Payment received from Northwind Ltd.", "Filed"),
    ("Lena Fischer", "2:26 AM", "Contract redlines attached", "Sorry for the hour, Berlin time. Two small changes.", "Draft ready"),
]
STORM_NAMES = ["Marco Ruiz", "Tom Becker", "Aiko Mori", "Sam Ortiz", "Raj Patel", "Chloe Martin", "Ben Adler",
               "Nora Kim", "Owen Hale", "Iris Lund", "Femi Ade", "Jo Park", "Luca Bassi", "Mei Chen", "Kai Weber",
               "Ana Silva", "Tariq Aziz", "Zoe Grant"]
STORM_SNIPS = ["Please remove me from this list.", "Same, please remove me.", "Why am I on this?",
               "Please stop replying all.", "+1, remove me", "Who added this list?", "Unsubscribe",
               "Replying all to say stop replying all."]


def storm_subject(k):
    return "Re: " * min(1 + k // 3, 5) + "Fwd: Offsite logistics"


# --------------------------------------------------------------------------------------------
def hero():
    scenes = []
    W = 1200
    X0 = (1920 - W) // 2

    # S1 · 0–2 s · frame 0 poster: 23:58, the first email lands and is read (macro)
    s1 = {"duration": 2.0, "layers": [ground(0.0)]}
    s1["layers"] += clock([(0, 23 * 60 + 58), (2.0, 23 * 60 + 59)], 2.0, size="78vh", x="50%", y="47%")
    s1["layers"].append(row(X0, 456, *NIGHT[0][:4], read=1.05, tag=NIGHT[0][4], arrive=0.0, enter=False,
                            extra={"animate": [{"from": {"y": 520, "opacity": 0.0}, "to": {"y": 456, "opacity": 1},
                                                "duration": 0.45, "ease": "outExpo", "at": 0.0}]}))
    s1["camera"] = {"animate": [{"from": {"zoom": 1.62, "x": 0, "y": 0}, "to": {"zoom": 1.5, "x": 0, "y": 0},
                                 "duration": 2.0, "ease": "inOutCubic"}]}
    scenes.append(s1)

    # S2 · 2–6 s · the night runs; rows land one by one, each read; claim on the left
    s2 = {"duration": 4.0, "layers": [ground(0.0, 0.04, 4.0)]}
    s2["layers"] += clock([(0, 23 * 60 + 59), (3.6, 2 * 60 + 26), (4.0, 2 * 60 + 27)], 4.0, size="78vh", x="50%", y="47%")
    LX = 820
    arrivals = [0.0, 0.85, 1.6, 2.5]
    for k, (sn, wh, sub, sni, tag) in enumerate(NIGHT):
        y = 150 + k * 196
        s2["layers"].append(row(LX, y, sn, wh, sub, sni, read=arrivals[k] + (0.45 if k else 0.0), tag=tag,
                                tag_amber=(tag == "Draft ready"), w=1000, arrive=arrivals[k], enter=k > 0))
    s2["layers"].append(claim("You sleep.\nIt reads.", 96, 360, "15vh", N_TEXT, start=0.35))
    s2["camera"] = {"animate": [{"from": {"zoom": 1.05, "y": -20}, "to": {"zoom": 1.0, "y": 10}, "duration": 4.0,
                                 "ease": "inOutCubic"}]}
    scenes.append(s2)

    # S3 · 6–10 s · 03:12 reply-all storm: a column that keeps growing, then Relay folds it to one row
    s3 = {"duration": 4.0, "layers": [ground(0.04)]}
    s3["layers"] += clock([(0, 3 * 60 + 12), (2.6, 3 * 60 + 31), (4.0, 3 * 60 + 33)], 4.0, size="78vh", x="50%", y="47%")
    N = 18
    PITCH = 186
    storm = []
    for k in range(N):
        sender = STORM_NAMES[k] + (", Priya, +%d" % (k + 20) if k % 4 == 0 else "")
        storm.append(row(X0, 120 + k * PITCH, sender, "3:%02d AM" % (12 + k), storm_subject(k), STORM_SNIPS[k % 8],
                         arrive=0.11 * k, enter=True))
    # the list scrolls up as it fills (one parent group), then is folded by Relay at 2.7 s
    col = group(0, 0, storm, animate=[
        {"from": {"y": 0}, "to": {"y": -(N - 4) * PITCH}, "duration": 2.4, "ease": "inOutCubic", "at": 0.15},
        {"to": {"scaleY": 0.0, "opacity": 0.0}, "duration": 0.35, "ease": "inExpo", "at": 2.65}])
    s3["layers"].append(col)
    # amber bracket that gathers the thread
    s3["layers"].append({"shape": "path", "d": "M0 0 L0 820", "stroke": AMBER, "strokeWidth": 4, "x": X0 - 40,
                         "y": 130, "anchor": "topLeft", "start": 2.2, "end": 3.0,
                         "animate": [{"from": {"trimEnd": 0}, "to": {"trimEnd": 1}, "duration": 0.4, "ease": "outExpo"}]})
    s3["layers"].append(row(X0, 456, "Offsite logistics · 41 replies", "3:31 AM", "Reply-all thread, nothing needs you",
                            "Relay muted it and left it out of your brief.", read=3.05, tag="Muted", tag_amber=False,
                            arrive=2.95, enter=False,
                            extra={"animate": [{"from": {"scaleY": 0.2, "opacity": 0}, "to": {"scaleY": 1, "opacity": 1},
                                                "duration": 0.45, "ease": "outExpo"}]}))
    s3["camera"] = {"animate": [{"from": {"zoom": 1.0}, "to": {"zoom": 1.06}, "duration": 2.6, "ease": "inOutCubic"},
                                {"to": {"zoom": 1.45, "x": 0}, "duration": 0.6, "ease": "outExpo", "at": 2.9}]}
    scenes.append(s3)

    # S4 · 10–14 s · the clock races to 06:59; first light; then the hard stop (13–14 s, silence)
    s4 = {"duration": 4.0, "layers": [ground(0.06, 0.62, 3.0, "inOutCubic")]}
    s4["layers"] += clock([(0, 3 * 60 + 33), (3.0, 6 * 60 + 59), (4.0, 6 * 60 + 59)], 4.0, size="62vh",
                          x="50%", y="50%", color="#FFFFFF30")
    late = [("Delta", "5:02 AM", "Check-in is open for DL 412", "Seat 14C. Boarding 10:40.", "Filed", 0.5),
            ("Ravi Menon", "6:18 AM", "Quick question", "Is the 10:00 still on? Running 5 late.", "Draft ready", 1.6)]
    for k, (sn, wh, sub, sni, tag, a) in enumerate(late):
        s4["layers"].append(row(X0 + (-260 if k == 0 else 260), 120 if k == 0 else 760, sn, wh, sub, sni,
                                read=a + 0.35, tag=tag, tag_amber=(tag == "Draft ready"), w=1200, arrive=a,
                                extra={"end": 3.0, "exit": {"preset": "fadeOut", "duration": 0.25}}))
    s4["camera"] = {"animate": [{"from": {"zoom": 1.0}, "to": {"zoom": 1.18}, "duration": 4.0, "ease": "inOutCubic"}]}
    scenes.append(s4)

    # S5 · 14–16 s · 07:00 (the hit): daylight; the brief arrives as one page
    s5 = {"duration": 2.0, "layers": [ground(0.75, 1.0, 0.5, "outExpo")]}
    s5["layers"] += clock([(0, 7 * 60), (2.0, 7 * 60)], 2.0, size="62vh", x="50%", y="50%", color="#14181F14")
    BW = 1100
    BX = (1920 - BW) // 2
    brief = [
        rect(0, 0, BW, 760, DAY_CARD, 36, stroke="#14181F10", strokeWidth=2,
             shadow={"color": "#5A4A3040", "blur": 60, "y": 24}),
        text("Good morning.", 64, 56, 64, D_TEXT, 700),
        text("Your brief · 2 min", 64, 140, 32, D_MUTE, 500),
        rect(64, 210, BW - 128, 2, "#14181F14"),
        text("Needs you", 64, 244, 28, "#A0650A", 700),
        text("Dana · Q3 numbers to the board before 9", 64, 296, 32, D_SUB, 550),
        text("Priya · Thursday for the design review?", 64, 352, 32, D_SUB, 550),
        text("Lena · Two contract redlines", 64, 408, 32, D_SUB, 550),
        text("Ravi · Is 10:00 still on?", 64, 464, 32, D_SUB, 550),
        text("4 replies drafted", BW - 64, 244, 28, "#A0650A", 700, anchor="topRight"),
        rect(64, 530, BW - 128, 2, "#14181F14"),
        text("Handled", 64, 562, 28, D_MUTE, 700),
        text("Invoice paid · Flight check-in · Offsite thread muted", 64, 614, 32, D_MUTE, 500),
    ]
    s5["layers"].append(group(BX, 160, brief, start=0.1,
                              animate=[{"from": {"y": 420, "scale": 0.92, "opacity": 0}, "to": {"y": 160, "scale": 1, "opacity": 1},
                                        "duration": 0.5, "ease": "outExpo"}]))
    s5["camera"] = {"animate": [{"from": {"zoom": 1.0}, "to": {"zoom": 1.05}, "duration": 2.0, "ease": "inOutCubic"}],
                    "shake": {"amount": 4, "at": 0.0, "duration": 0.2}}
    scenes.append(s5)

    # S6 · 16–22 s · the magic shot: one thread, its drafted reply, Send (unbroken)
    s6 = {"duration": 6.0, "layers": [ground(1.0)]}
    s6["layers"] += clock([(0, 7 * 60), (6.0, 7 * 60 + 1)], 6.0, size="62vh", x="50%", y="50%", color="#14181F10")

    def thread(sender, subj, body, draft, sent_at, y0, dur_in=0.0):
        TW = 1240
        L = [
            rect(0, 0, TW, 470, DAY_CARD, 32, stroke="#14181F10", strokeWidth=2,
                 shadow={"color": "#5A4A3033", "blur": 50, "y": 18}),
            text(sender, 52, 40, 38, D_TEXT, 650),
            text(subj, 52, 92, 32, D_SUB, 550),
            text(body, 52, 140, 28, D_MUTE, 400),
            rect(52, 196, TW - 104, 196, "#FFF4E0", 22),
            text("Draft reply", 84, 220, 28, "#B9760F", 700),
            text(draft, 84, 270, 32, D_TEXT, 500, maxWidth=TW - 420),
            # Send button
            rect(TW - 268, 300, 184, 72, AMBER, 36, start=0, end=sent_at),
            text("Send", TW - 176, 336, 32, D_TEXT, 700, anchor="center", end=sent_at),
            rect(TW - 268, 300, 184, 72, "#14181F", 36, start=sent_at),
            text("Sent ✓", TW - 176, 336, 32, "#FBFAF7", 700, anchor="center", start=sent_at),
        ]
        return group((1920 - TW) // 2, y0, L, start=dur_in)

    t1 = thread("Dana Whitfield", "One small thing before tomorrow",
                "Can you send the Q3 numbers to the board before 9? I'll be offline until 10.",
                "Done — Q3 numbers are attached and going to the board this morning.", 2.2, 300)
    t1["animate"] = [{"to": {"y": -700}, "duration": 0.5, "ease": "inExpo", "at": 2.75}]
    t2 = thread("Priya Shah", "Design review", "Does Thursday work? Happy to move if not.",
                "Thursday works. 2:30 at your desk?", 3.9, 1150)
    t2["animate"] = [{"to": {"y": 300}, "duration": 0.5, "ease": "outExpo", "at": 2.85},
                     {"to": {"y": -700}, "duration": 0.45, "ease": "inExpo", "at": 4.25}]
    t3 = thread("Lena Fischer", "Contract redlines attached", "Sorry for the hour, Berlin time. Two small changes.",
                "Both changes look fine to me. Sending the signed copy today.", 5.3, 1150)
    t3["animate"] = [{"to": {"y": 300}, "duration": 0.45, "ease": "outExpo", "at": 4.35}]
    s6["layers"] += [t1, t2, t3]
    # one cursor, eased arcs, settles before each click
    BXs = (1920 - 1240) // 2 + 1240 - 176
    BYs = 300 + 336
    s6["layers"].append({"component": "cursor", "x": 0, "y": 0, "anchor": "topLeft", "width": 1920, "height": 1080,
                         "props": {"path": [[1500, 980], [BXs, BYs], [BXs, BYs], [BXs + 40, BYs + 120], [BXs, BYs],
                                            [BXs, BYs], [BXs + 30, BYs + 110], [BXs, BYs], [BXs, BYs]],
                                   "times": [0.3, 1.75, 2.2, 2.6, 3.5, 3.9, 4.1, 4.95, 5.3],
                                   "clicks": [2.2, 3.9, 5.3], "size": 44, "style": "arrow", "color": "#14181F",
                                   "ripple": AMBER, "trail": False}})
    s6["camera"] = {"animate": [
        {"from": {"zoom": 1.5, "x": 0, "y": -5}, "to": {"zoom": 1.4, "x": 0, "y": -5}, "duration": 2.0, "ease": "inOutCubic"},
        {"to": {"zoom": 1.25, "x": 0, "y": -5}, "duration": 2.6, "ease": "inOutCubic", "at": 2.4}]}
    # honest label
    scenes.append(s6)

    # S7 · 22–26 s · Approve. Send. — the brief's checklist turns to sent, inbox empties
    s7 = {"duration": 4.0, "layers": [ground(1.0)]}
    s7["layers"] += clock([(0, 7 * 60 + 1), (4.0, 7 * 60 + 2)], 4.0, size="62vh", x="50%", y="50%", color="#14181F10")
    s7["layers"].append(claim("Approve.\nSend.", 96, 290, "20vh", D_TEXT, start=0.0))
    items = ["Dana · Q3 numbers", "Priya · Thursday 2:30", "Lena · Signed copy today", "Ravi · 10:00 still on"]
    for k, it in enumerate(items):
        at = 0.6 + k * 0.5
        y = 260 + k * 150
        s7["layers"].append(group(1060, y, [
            rect(0, 0, 780, 120, DAY_CARD, 26, stroke="#14181F10", strokeWidth=2),
            text(it, 40, 38, 36, D_TEXT, 600),
            rect(780 - 196, 24, 160, 72, AMBER, 36, end=at),
            text("Send", 780 - 116, 60, 30, D_TEXT, 700, anchor="center", end=at),
            rect(780 - 196, 24, 160, 72, "#14181F", 36, start=at),
            text("Sent ✓", 780 - 116, 60, 30, "#FBFAF7", 700, anchor="center", start=at),
        ], start=0.1 + k * 0.08, enter={"preset": "fadeUp", "duration": 0.4, "distance": 30, "ease": "outExpo"},
            animate=[{"to": {"x": 2000, "opacity": 0}, "duration": 0.35, "ease": "inExpo", "at": 2.9 + k * 0.08}]))
    s7["camera"] = {"animate": [{"from": {"zoom": 1.08}, "to": {"zoom": 1.0}, "duration": 4.0, "ease": "inOutCubic"}]}
    scenes.append(s7)

    # S8 · 26–32 s · ending: 07:02, empty inbox, the name
    s8 = {"duration": 6.0, "layers": [ground(1.0)]}
    s8["layers"] += clock([(0, 7 * 60 + 2), (6.0, 7 * 60 + 2)], 6.0, size="62vh", x="50%", y="40%", color="#14181F1A")
    s8["layers"].append(group(560, 600, [
        rect(0, 0, 800, 120, DAY_CARD, 26, stroke="#14181F10", strokeWidth=2),
        {"shape": "circle", "x": 52, "y": 60, "width": 14, "height": 14, "fill": AMBER, "anchor": "center"},
        text("Inbox · nothing waiting", 84, 40, 36, D_TEXT, 600),
    ], start=0.2, enter={"preset": "fadeUp", "duration": 0.5, "distance": 30, "ease": "outExpo"}))
    s8["layers"].append(claim("Relay", 960, 870, "16vh", D_TEXT, start=1.2, anchor="center", enter={"preset": "fadeUp", "duration": 0.5, "distance": 30}))
    s8["layers"].append(text("Reads overnight. Ready by morning.", 960, 985, 34, D_MUTE, 500, anchor="center",
                             start=1.6, enter={"preset": "fadeUp", "duration": 0.5, "distance": 20}))
    s8["camera"] = {"animate": [{"from": {"zoom": 1.06}, "to": {"zoom": 1.0}, "duration": 6.0, "ease": "inOutCubic"}]}
    scenes.append(s8)

    spec = {
        "size": "16:9", "fps": FPS, "background": INK,
        "intent": {"pace": "fast"},
        "music": music(16, stop_bar=8),
        "sound": "auto",
        "post": {"grain": 0.02, "vignette": 0.12},
        "scenes": scenes,
    }
    return spec


def music(bars, stop_bar):
    pre = stop_bar - 1
    return {"style": "pop", "bpm": 120, "key": "D", "mood": "bright",
            "sections": [{"name": "intro", "bars": 3, "energy": 0.35},
                         {"name": "build", "bars": pre - 3, "energy": 0.6},
                         {"name": "drop", "bars": bars - pre - 2, "energy": 1.0, "stop": 2},
                         {"name": "outro", "bars": 2, "energy": 0.5}],
            "hits": ["@bar%d" % stop_bar], "end": "ring"}


if __name__ == "__main__":
    which = sys.argv[1] if len(sys.argv) > 1 else "hero"
    if which == "hero":
        json.dump(hero(), open(os.path.join(OUT, "spec.json"), "w"), indent=1)
        print("spec.json")
