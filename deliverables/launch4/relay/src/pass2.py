#!/usr/bin/env python3
"""Pass 2 of the Relay launch film: the night's inbox lives INSIDE the clock numerals.

Derivation: Relay works inside the night -> the numerals are a window (matte) onto the
scrolling inbox, lit like a screen in a dark room. At 07:00 the inbox drains out of the
numerals and the brief rises; at the end the numerals and the name are filled with the
morning's sent replies.

usage: python3 src/pass2.py [hero|vertical]   -> spec.json / spec-9x16.json
"""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from build import (ground, clock, group, text, rect, row, claim, music, NIGHT, STORM_NAMES, STORM_SNIPS,
                   storm_subject, AMBER, DAY_CARD, D_TEXT, D_SUB, D_MUTE, N_TEXT, N_MUTE, INK, UI, DISP, FPS, OUT)

M = lambda h, m: h * 60 + m
SCREEN = "#DCE3EC"     # the inbox's own background: the light inside the numerals at night
INKF = "#14181F"

POOL = NIGHT + [
    ("Stripe", "1:07 AM", "Invoice INV-2291 paid", "Payment received from Northwind Ltd.", "Filed"),
    ("GitHub", "1:52 AM", "[relay/app] CI passed on main", "All checks have passed.", "Filed"),
    ("Mom", "12:15 AM", "Sunday?", "Are you still coming for lunch? Bring the good bread.", "Draft ready"),
    ("Delta", "5:02 AM", "Check-in is open for DL 412", "Seat 14C. Boarding 10:40.", "Filed"),
    ("Ravi Menon", "6:18 AM", "Quick question", "Is the 10:00 still on? Running 5 late.", "Draft ready"),
    ("The Overnight", "4:00 AM", "Issue 212: what we read this week", "Six links and one long read.", "Skim"),
]
SENT = [
    ("To Dana Whitfield", "7:01 AM", "Re: One small thing before tomorrow", "Done — I'll send the Q3 numbers to the board before 9."),
    ("To Priya Shah", "7:01 AM", "Re: Design review", "Thursday works. 2:30 at your desk?"),
    ("To Lena Fischer", "7:01 AM", "Re: Contract redlines attached", "Both changes look fine to me. Signed copy today."),
    ("To Ravi Menon", "7:02 AM", "Re: Quick question", "Still on. See you at 10."),
    ("To Mom", "7:02 AM", "Re: Sunday?", "Yes! Bringing the good bread."),
]


def fill(kind, scroll, dur, matte, bg=SCREEN, at=0.0, ease="inOutCubic", extra_anim=None, read=True, w=900):
    """Rows tiled in 2 columns, scrolled by `scroll` px over `dur`, shown through `matte`."""
    rows = []
    need = int((1080 + abs(scroll)) / 186) + 3
    for k in range(need):
        for c in range(2):
            x, y = 40 + c * 940, -186 + k * 186
            j = k * 2 + c
            if kind == "night":
                sn, wh, sub, sni, tag = POOL[(j * 3 + k) % len(POOL)]
                rows.append(row(x, y, sn, wh, sub, sni, read=0.0 if read else None, tag=tag,
                                tag_amber=(tag == "Draft ready"), w=w, enter=False, night=False))
            elif kind == "storm":
                sender = STORM_NAMES[j % len(STORM_NAMES)] + (", Priya, +%d" % (20 + j) if j % 5 == 0 else "")
                rows.append(row(x, y, sender, "3:%02d AM" % (12 + j % 40), storm_subject(j), STORM_SNIPS[j % 8],
                                w=w, enter=False, night=False))
            elif kind == "sent":
                sn, wh, sub, sni = SENT[(j + k) % len(SENT)]
                rows.append(row(x, y, sn, wh, sub, sni, read=0.0, tag="Sent ✓", tag_amber=False, w=w,
                                enter=False, night=True))
    anim = [{"from": {"y": 0}, "to": {"y": -scroll}, "duration": dur, "ease": ease, "at": at}]
    if extra_anim:
        anim += extra_anim
    return group(0, 0, [rect(-200, -200, 2320, 1480, bg)] + rows, matte=matte, animate=anim)


def clockgroup(id_, keys, dur, size="80vh", y="50%", color="#FFFFFF", **kw):
    g = {"id": id_, "x": 0, "y": 0, "anchor": "topLeft", "width": 1920, "height": 1080,
         "layers": clock(keys, dur, size=size, y=y, color=color)}
    g.update(kw)
    return g


def halo(keys, dur, size="80vh", y="50%", color="#AFC6EE", opacity=0.22, blur=60):
    return {"x": 0, "y": 0, "anchor": "topLeft", "width": 1920, "height": 1080, "opacity": opacity, "blur": blur,
            "layers": clock(keys, dur, size=size, y=y, color=color)}


def outline_matted(layers, color):
    """After every matted fill, draw a hairline outline of its matte glyphs so they read as type."""
    ids = {L.get("id"): L for L in layers if isinstance(L, dict) and L.get("id")}
    out = []
    for L in layers:
        out.append(L)
        m = L.get("matte") if isinstance(L, dict) else None
        if m and m in ids and L.get("opacity", 1) > 0 and ids[m].get("opacity", 1) > 0:
            src = ids[m]
            kids = src["layers"] if "layers" in src else [src]
            ol = []
            for t in kids:
                t2 = {k: v for k, v in t.items() if k != "id"}
                t2["color"] = "#00000000"
                t2["stroke"] = {"color": color, "width": 3}
                ol.append(t2)
            g = {k: v for k, v in src.items() if k not in ("id", "layers", "text", "color")}
            if "layers" in src:
                g["layers"] = ol
                out.append(g)
            else:
                out.append(ol[0])
    return out


def scene(dur, layers, camera=None, **kw):
    s = {"duration": dur, "layers": layers}
    s["layers"] = outline_matted(layers, "#FFFFFF8C" if not any(
        isinstance(L, dict) and L.get("matte") and '"Sent' in json.dumps(L) for L in layers) else "#14181FCC")
    if camera:
        s["camera"] = camera
    s.update(kw)
    return s


def cam(*moves, shake=None):
    c = {"animate": list(moves)}
    if shake:
        c["shake"] = shake
    return c


def mv(frm, to, dur, ease="inOutCubic", at=0.0):
    d = {"to": to, "duration": dur, "ease": ease, "at": at}
    if frm:
        d["from"] = frm
    return d


def free_card(x, y, data, read, tag, w=1200, night_light=True, **kw):
    """A single row lifted out of the inbox, lit (screen light), for macro shots."""
    sn, wh, sub, sni = data[:4]
    r = row(x, y, sn, wh, sub, sni, read=read, tag=tag, tag_amber=(tag == "Draft ready"), w=w, enter=False,
            night=False, extra=kw)
    if night_light:
        r["layers"][0]["shadow"] = {"color": "#9DB8E866", "blur": 90, "y": 0}
    return r


def stage(id_, keys, dur, levels):
    """The morning numerals: an ink hairline (added by outline_matted) that fills with sent replies.
    levels: [(t, opacity)] steps, each eased in 0.25 s."""
    f = fill("sent", 60, dur, id_, bg=INKF, ease="linear")
    f["opacity"] = levels[0][1]
    for t, o in levels[1:]:
        f["animate"].append({"to": {"opacity": o}, "duration": 0.25, "ease": "outExpo", "at": t})
    return [clockgroup(id_, keys, dur), f]


def thread(sender, subj, body, draft, sent_at, y0, start=0.0):
    TW = 1240
    L = [
        rect(0, 0, TW, 470, DAY_CARD, 32, stroke="#14181F10", strokeWidth=2,
             shadow={"color": "#5A4A3033", "blur": 50, "y": 18}),
        text(sender, 52, 40, 38, D_TEXT, 650),
        text(subj, 52, 92, 32, D_SUB, 550),
        text(body, 52, 140, 28, D_MUTE, 400),
        rect(52, 196, TW - 104, 196, "#FFF4E0", 22),
        text("Draft reply", 84, 220, 28, "#A0650A", 700),
        text(draft, 84, 270, 32, D_TEXT, 500, maxWidth=TW - 420),
        rect(TW - 268, 300, 184, 72, AMBER, 36, end=sent_at),
        text("Send", TW - 176, 336, 32, D_TEXT, 700, anchor="center", end=sent_at),
        rect(TW - 268, 300, 184, 72, INKF, 36, start=sent_at,
             animate=[{"from": {"scale": 0.94}, "to": {"scale": 1}, "duration": 0.25, "ease": "outExpo"}]),
        text("Sent ✓", TW - 176, 336, 32, "#FBFAF7", 700, anchor="center", start=sent_at),
    ]
    return group((1920 - TW) // 2, y0, L, start=start)


def cursor(points, times, clicks):
    return {"component": "cursor", "x": 0, "y": 0, "anchor": "topLeft", "width": 1920, "height": 1080,
            "props": {"path": points, "times": times, "clicks": clicks, "size": 44, "style": "arrow",
                      "color": INKF, "ripple": AMBER, "trail": False}}


def hero():
    S = []
    # 1 · 0–1.6 · frame 0: one email, lit like a screen in the dark, read by Relay
    k1 = [(0, M(23, 58)), (1.6, M(23, 58))]
    S.append(scene(1.6, [
        ground(0.0),
        halo(k1, 1.6, opacity=0.35, blur=24),
        free_card(360, 456, NIGHT[0], read=0.8, tag="Draft ready",
                  animate=[mv({"y": 520, "opacity": 0}, {"y": 456, "opacity": 1}, 0.4, "outExpo")]),
    ], cam(mv({"zoom": 1.62}, {"zoom": 1.5}, 1.6))))

    # 2 · 1.6–3.2 · pull back: that email is one of many, inside the numerals 23:58
    k2 = [(0, M(23, 58)), (1.6, M(23, 59))]
    S.append(scene(1.6, [
        ground(0.0), halo(k2, 1.6),
        clockgroup("c2", k2, 1.6),
        fill("night", 120, 1.6, "c2"),
    ], cam(mv({"zoom": 2.6, "x": 260, "y": -30}, {"zoom": 1.0, "x": 0, "y": 0}, 1.2, "inOutCubic"))))

    # 3 · 3.2–5.0 · the night rolls; "You sleep. It reads."
    k3 = [(0, M(23, 59)), (1.8, M(25, 7))]
    S.append(scene(1.8, [
        ground(0.0), halo(k3, 1.8, y="44%", size="70vh"),
        clockgroup("c3", k3, 1.8, size="70vh", y="44%"),
        fill("night", 520, 1.8, "c3"),
        claim("You sleep. It reads.", 960, 960, "8vh", N_TEXT, start=0.1, anchor="center"),
    ], cam(mv({"zoom": 1.0}, {"zoom": 1.04}, 1.8))))

    # 4 · 5.0–6.4 · macro: 2:26 AM, Lena from Berlin, read; numerals blurred behind (depth)
    k4 = [(0, M(2, 26)), (1.4, M(2, 27))]
    S.append(scene(1.4, [
        ground(0.02),
        {**clockgroup("c4", k4, 1.4), "opacity": 0.0},
        {**fill("night", 60, 1.4, "c4"), "blur": 14, "opacity": 0.55},
        free_card(360, 456, NIGHT[3], read=0.45, tag="Draft ready",
                  animate=[mv({"y": 500, "opacity": 0}, {"y": 456, "opacity": 1}, 0.35, "outExpo")]),
    ], cam(mv({"zoom": 1.5, "x": 0}, {"zoom": 1.42, "x": 0}, 1.4))))

    # 5 · 6.4–8.4 · 03:12 reply-all storm floods the numerals
    k5 = [(0, M(3, 12)), (2.0, M(3, 29))]
    S.append(scene(2.0, [
        ground(0.03), halo(k5, 2.0, opacity=0.3),
        clockgroup("c5", k5, 2.0),
        fill("storm", 2600, 2.0, "c5", ease="inCubic"),
    ], cam(mv({"zoom": 1.0}, {"zoom": 1.12}, 2.0, "inCubic"), shake={"amount": 3, "at": 1.4, "duration": 0.6})))

    # 6 · 8.4–9.6 · Relay reads the whole thread: one row, muted (the night's first payoff)
    S.append(scene(1.2, [
        ground(0.03),
        {**clockgroup("c6", [(0, M(3, 31)), (1.2, M(3, 31))], 1.2), "opacity": 0.0},
        {**fill("storm", 40, 1.2, "c6"), "blur": 16, "opacity": 0.5},
        free_card(360, 456, ("Offsite logistics · 41 replies", "3:31 AM", "Reply-all thread, nothing needs you",
                             "Muted and left out of your brief."), read=0.3, tag="Muted",
                  animate=[mv({"scaleY": 0.3, "opacity": 0}, {"scaleY": 1, "opacity": 1}, 0.3, "outExpo")]),
    ], cam(mv({"zoom": 1.55}, {"zoom": 1.45}, 1.2, "outExpo"), shake={"amount": 5, "at": 0.0, "duration": 0.2})))

    # 7 · 9.6–12.0 · the night runs out; first light
    k7 = [(0, M(3, 33)), (2.4, M(6, 31))]
    S.append(scene(2.4, [
        ground(0.04, 0.45, 2.4), halo(k7, 2.4, opacity=0.18),
        clockgroup("c7", k7, 2.4),
        fill("night", 900, 2.4, "c7"),
    ], cam(mv({"zoom": 1.12, "y": 20}, {"zoom": 1.0, "y": 0}, 2.4))))

    # 8 · 12.0–14.0 · 06:59, then everything stops (the hard stop is in the music, 13–14 s)
    k8 = [(0, M(6, 45)), (1.0, M(6, 59)), (2.0, M(6, 59))]
    S.append(scene(2.0, [
        ground(0.45, 0.6, 1.0), halo(k8, 2.0, opacity=0.15),
        clockgroup("c8", k8, 2.0),
        fill("night", 240, 1.0, "c8", ease="outCubic"),
    ], cam(mv({"zoom": 1.0}, {"zoom": 1.07}, 1.0, "outCubic"))))

    # 9 · 14.0–15.6 · 07:00 (the hit): the inbox drains out of the numerals; daylight
    k9 = [(0, M(7, 0)), (1.6, M(7, 0))]
    S.append(scene(1.6, [
        ground(0.7, 1.0, 0.6, "outExpo"),
        {"x": 0, "y": 0, "anchor": "topLeft", "width": 1920, "height": 1080, "start": 0.2,
         "enter": {"preset": "fade", "duration": 0.25},
         "layers": clock(k9, 1.6, color=INKF)},
        clockgroup("c9", k9, 1.6),
        fill("night", 0, 0.1, "c9", extra_anim=[mv(None, {"y": 1300}, 0.35, "inExpo", 0.0)]),
    ], cam(mv({"zoom": 1.22}, {"zoom": 1.0}, 0.5, "outExpo"), shake={"amount": 6, "at": 0.0, "duration": 0.25})))

    S[-1]["layers"] = [L for L in S[-1]["layers"] if not (isinstance(L, dict) and "stroke" in json.dumps(L)[:400] and L.get("layers") and L["layers"][0].get("stroke"))]

    # 10 · 15.6–19.0 · the brief rises out of the numerals
    BW = 1100
    BX = (1920 - BW) // 2
    brief = [
        rect(0, 0, BW, 700, DAY_CARD, 36, stroke="#14181F10", strokeWidth=2, shadow={"color": "#5A4A3040", "blur": 60, "y": 24}),
        text("Good morning.", 64, 56, 64, D_TEXT, 700),
        text("Your brief · 2 min", 64, 140, 32, D_MUTE, 500),
        rect(64, 210, BW - 128, 2, "#14181F14"),
        text("Needs you", 64, 244, 28, "#A0650A", 700),
        text("4 replies drafted", BW - 64, 244, 28, "#A0650A", 700, anchor="topRight"),
        text("Dana · Q3 numbers to the board before 9", 64, 296, 32, D_SUB, 550),
        text("Priya · Thursday for the design review?", 64, 348, 32, D_SUB, 550),
        text("Lena · Two contract redlines", 64, 400, 32, D_SUB, 550),
        text("Ravi · Is 10:00 still on?", 64, 452, 32, D_SUB, 550),
        rect(64, 520, BW - 128, 2, "#14181F14"),
        text("Handled", 64, 550, 28, D_MUTE, 700),
        text("Offsite thread muted · Invoice paid · Check-in open", 64, 600, 32, D_MUTE, 500),
    ]
    S.append(scene(1.7, [
        ground(1.0),
        *stage("c10", [(0, M(7, 0)), (1.7, M(7, 0))], 1.7, [(0, 0.08)]),
        group(BX, 190, brief, animate=[mv({"y": 700, "opacity": 0}, {"y": 190, "opacity": 1}, 0.5, "outExpo")]),
    ], cam(mv({"zoom": 1.0, "y": 0}, {"zoom": 1.08, "y": -10}, 1.7, "inOutCubic"))))
    S.append(scene(1.7, [
        ground(1.0),
        *stage("c10b", [(0, M(7, 0)), (1.7, M(7, 0))], 1.7, [(0, 0.08)]),
        group(BX, 190, brief),
    ], cam(mv({"zoom": 1.6, "x": -60, "y": 10}, {"zoom": 1.52, "x": -60, "y": 10}, 1.7, "inOutCubic"))))

    # 11 · 17.6–20.4 · the magic shot, unbroken: Dana's 11:58 PM ask, its drafted reply, Send
    BXs, BYs = (1920 - 1240) // 2 + 1240 - 176, 300 + 336
    t1 = thread("Dana Whitfield", "One small thing before tomorrow",
                "Can you send the Q3 numbers to the board before 9? I'll be offline until 10.",
                "Done — I'll send the Q3 numbers to the board before 9.", 2.05, 300)
    t1["animate"] = [mv(None, {"y": -700}, 0.4, "inExpo", 2.4)]
    S.append(scene(2.8, [
        ground(1.0),
        *stage("c11", [(0, M(7, 0)), (2.0, M(7, 0)), (2.05, M(7, 1)), (2.8, M(7, 1))], 2.8, [(0, 0.08), (2.0, 0.3)]),
        t1,
        cursor([[1560, 1000], [BXs + 6, BYs + 4], [BXs, BYs], [BXs, BYs]], [0.2, 1.4, 1.7, 2.8], [2.0]),
    ], cam(mv({"zoom": 1.5}, {"zoom": 1.42}, 1.6), mv(None, {"zoom": 1.9, "x": 330, "y": 120}, 0.5, "inOutCubic", 1.45),
           mv(None, {"zoom": 1.5, "x": 0, "y": 0}, 0.4, "inOutCubic", 2.3))))

    # 12 · 20.4–21.6 · Priya
    t2 = thread("Priya Shah", "Design review", "Does Thursday work? Happy to move if not.",
                "Thursday works. 2:30 at your desk?", 0.75, 300)
    t2["animate"] = [mv({"y": 1150}, {"y": 300}, 0.3, "outExpo"), mv(None, {"y": -700}, 0.3, "inExpo", 0.95)]
    S.append(scene(1.2, [ground(1.0), *stage("c12", [(0, M(7, 1)), (1.2, M(7, 1))], 1.2, [(0, 0.3), (0.75, 0.5)]), t2, cursor([[BXs + 40, BYs + 140], [BXs, BYs], [BXs, BYs]], [0.0, 0.5, 1.2], [0.75])],
                   cam(mv({"zoom": 1.4}, {"zoom": 1.45}, 1.2))))

    # 13 · 21.6–22.6 · Lena
    t3 = thread("Lena Fischer", "Contract redlines attached", "Sorry for the hour, Berlin time. Two small changes.",
                "Both changes look fine to me. Signed copy today.", 0.55, 300)
    t3["animate"] = [mv({"y": 1150}, {"y": 300}, 0.25, "outExpo"), mv(None, {"y": -700}, 0.25, "inExpo", 0.78)]
    S.append(scene(1.0, [ground(1.0), *stage("c13", [(0, M(7, 1)), (1.0, M(7, 1))], 1.0, [(0, 0.5), (0.55, 0.68)]), t3, cursor([[BXs + 30, BYs + 130], [BXs, BYs], [BXs, BYs]], [0.0, 0.38, 1.0], [0.55])],
                   cam(mv({"zoom": 1.32}, {"zoom": 1.36}, 1.0))))

    # 14 · 22.6–25.0 · Approve. Send. — the whole morning, on the beat (climax)
    items = ["Dana · Q3 numbers", "Priya · Thursday 2:30", "Lena · Signed copy today", "Ravi · 10:00 still on", "Mom · Sunday lunch"]
    L14 = [ground(1.0),
           claim("Approve.\nSend.", 96, 300, "20vh", D_TEXT, start=0.0)]
    for k, it in enumerate(items):
        at = 0.5 + k * 0.25
        L14.append(group(1060, 180 + k * 150, [
            rect(0, 0, 780, 120, DAY_CARD, 26, stroke="#14181F10", strokeWidth=2),
            text(it, 40, 38, 36, D_TEXT, 600),
            rect(584, 24, 160, 72, AMBER, 36, end=at),
            text("Send", 664, 60, 30, D_TEXT, 700, anchor="center", end=at),
            rect(584, 24, 160, 72, INKF, 36, start=at,
                 animate=[{"from": {"scale": 0.92}, "to": {"scale": 1}, "duration": 0.2, "ease": "outExpo"}]),
            text("Sent ✓", 664, 60, 30, "#FBFAF7", 700, anchor="center", start=at),
        ], start=k * 0.05, enter={"preset": "fadeUp", "duration": 0.3, "distance": 30, "ease": "outExpo"},
            animate=[mv(None, {"x": 2100}, 0.3, "inExpo", 1.95 + k * 0.05)]))
    S.append(scene(2.4, L14, cam(mv({"zoom": 1.06}, {"zoom": 1.0}, 2.4, "outCubic"))))

    # 15 · 25.0–28.0 · 07:02: the numerals now hold the morning's sent replies
    k15 = [(0, M(7, 2)), (2.6, M(7, 2))]
    S.append(scene(2.6, [
        ground(1.0),
        clockgroup("c15", k15, 2.6),
        fill("sent", 300, 2.6, "c15", bg=INKF, ease="inOutSine"),
    ], cam(mv({"zoom": 1.9, "x": -420, "y": 60}, {"zoom": 1.0, "x": 0, "y": 0}, 1.4, "inOutCubic"))))

    # 16 · 28.0–32.0 · the name, made of the same window
    S.append(scene(3.0, [
        ground(1.0),
        {"id": "nm", "text": "Relay", "font": DISP, "weight": 800, "size": "40vh", "letterSpacing": "-0.05em",
         "color": "#FFFFFF", "x": "50%", "y": "44%"},
        fill("sent", 240, 3.0, "nm", bg=INKF, ease="inOutSine"),
        text("Reads overnight. Ready by morning.", 960, 830, 44, D_TEXT, 600, anchor="center", start=0.5,
             enter={"preset": "fadeUp", "duration": 0.5, "distance": 20, "ease": "outExpo"}),
    ], cam(mv({"zoom": 1.12}, {"zoom": 1.0}, 1.2, "outExpo"))))

    return {"size": "16:9", "fps": FPS, "background": INK, "intent": {"pace": "fast"},
            "music": music(16, stop_bar=8), "sound": "auto", "post": {"grain": 0.02, "vignette": 0.12},
            "scenes": S}


if __name__ == "__main__":
    which = sys.argv[1] if len(sys.argv) > 1 else "hero"
    if which == "hero":
        json.dump(hero(), open(os.path.join(OUT, "spec.json"), "w"), indent=1)
        print("spec.json", sum(s["duration"] for s in hero()["scenes"]))


# ------------------------------------------------------------------------------------------
# 9:16 cut, 16 s (8 bars at 120 bpm), composed for the vertical frame: the numerals stack.
VW, VH = 1080, 1920


def vclock_layers(keys, dur, size="38vh", y="46%", color="#FFFFFF"):
    L = clock(keys, dur, size=size, y=y, color=color)
    for t in L:
        t["text"] = t["text"].replace(":", "\n")
        t["lineHeight"] = 0.8
        t["x"] = "50%"
    return L


def vgroup(layers, **kw):
    g = {"x": 0, "y": 0, "anchor": "topLeft", "width": VW, "height": VH, "layers": layers}
    g.update(kw)
    return g


def vfill(kind, scroll, dur, matte, bg=SCREEN, ease="inOutCubic", extra_anim=None, night_rows=False):
    rows = []
    need = int((VH + abs(scroll)) / 186) + 3
    for k in range(need):
        y = -186 + k * 186
        if kind == "night":
            sn, wh, sub, sni, tag = POOL[(k * 3) % len(POOL)]
            rows.append(row(40, y, sn, wh, sub, sni, read=0.0, tag=tag, tag_amber=(tag == "Draft ready"), w=1000,
                            enter=False, night=False))
        elif kind == "storm":
            rows.append(row(40, y, STORM_NAMES[k % len(STORM_NAMES)], "3:%02d AM" % (12 + k % 40), storm_subject(k),
                            STORM_SNIPS[k % 8], w=1000, enter=False, night=False))
        else:
            sn, wh, sub, sni = SENT[k % len(SENT)]
            rows.append(row(40, y, sn, wh, sub, sni, read=0.0, tag="Sent ✓", tag_amber=False, w=1000, enter=False,
                            night=True))
    anim = [{"from": {"y": 0}, "to": {"y": -scroll}, "duration": dur, "ease": ease}]
    if extra_anim:
        anim += extra_anim
    return vgroup([rect(-200, -200, VW + 400, VH + 400, bg)] + rows, matte=matte, animate=anim)


def vcard(data, read, tag, y=860, **kw):
    sn, wh, sub, sni = data[:4]
    r = row(40, y, sn, wh, sub, sni, read=read, tag=tag, tag_amber=(tag == "Draft ready"), w=1000, enter=False,
            night=False, extra=kw)
    r["layers"][0]["shadow"] = {"color": "#9DB8E866", "blur": 90, "y": 0}
    return r


def vertical():
    S = []
    k = [(0, M(23, 58)), (1.2, M(23, 58))]
    S.append(scene(1.2, [ground(0.0), {**vgroup(vclock_layers(k, 1.2)), "opacity": 0.3, "blur": 24},
                         vcard(NIGHT[0], 0.6, "Draft ready",
                               animate=[mv({"y": 920, "opacity": 0}, {"y": 860, "opacity": 1}, 0.35, "outExpo")])],
                   cam(mv({"zoom": 1.05}, {"zoom": 1.0}, 1.2))))
    k = [(0, M(23, 58)), (1.4, M(25, 10))]
    S.append(scene(1.4, [ground(0.0), {**vgroup(vclock_layers(k, 1.4)), "opacity": 0.22, "blur": 50},
                         vgroup(vclock_layers(k, 1.4), id="v2"), vfill("night", 500, 1.4, "v2"),
                         claim("You sleep.\nIt reads.", 540, 1560, "6vh", N_TEXT, anchor="center", align="center",
                               start=0.15)],
                   cam(mv({"zoom": 2.2, "y": 0}, {"zoom": 1.0}, 0.9, "inOutCubic"))))
    k = [(0, M(3, 12)), (1.4, M(3, 29))]
    S.append(scene(1.4, [ground(0.03), {**vgroup(vclock_layers(k, 1.4)), "opacity": 0.3, "blur": 50},
                         vgroup(vclock_layers(k, 1.4), id="v3"), vfill("storm", 2400, 1.4, "v3", ease="inCubic")],
                   cam(mv({"zoom": 1.0}, {"zoom": 1.1}, 1.4, "inCubic"))))
    k = [(0, M(3, 31)), (1.0, M(6, 59)), (2.0, M(6, 59))]
    S.append(scene(2.0, [ground(0.05, 0.6, 1.0), {**vgroup(vclock_layers(k, 2.0)), "opacity": 0.15, "blur": 50},
                         vgroup(vclock_layers(k, 2.0), id="v4"), vfill("night", 800, 1.0, "v4", ease="outCubic")],
                   cam(mv({"zoom": 1.0}, {"zoom": 1.05}, 1.0))))
    k = [(0, M(7, 0)), (1.0, M(7, 0))]
    S.append(scene(1.0, [ground(0.7, 1.0, 0.5, "outExpo"),
                         vgroup(vclock_layers(k, 1.0, color=INKF), start=0.4, enter={"preset": "fade", "duration": 0.3}),
                         vgroup(vclock_layers(k, 1.0), id="v5"),
                         vfill("night", 0, 0.1, "v5", extra_anim=[mv(None, {"y": 2200}, 0.45, "inExpo", 0.05)])],
                   cam(mv({"zoom": 1.15}, {"zoom": 1.0}, 0.4, "outExpo"), shake={"amount": 6, "at": 0, "duration": 0.25})))
    # magic shot
    TW = 1000
    L = [rect(0, 0, TW, 560, DAY_CARD, 32, stroke="#14181F10", strokeWidth=2, shadow={"color": "#5A4A3033", "blur": 50, "y": 18}),
         text("Dana Whitfield", 48, 36, 50, D_TEXT, 650),
         text("11:58 PM · Q3 numbers before 9?", 48, 104, 38, D_SUB, 550),
         rect(48, 180, TW - 96, 330, "#FFF4E0", 22),
         text("Draft reply", 80, 204, 34, "#A0650A", 700),
         text("Done — I'll send the Q3 numbers to the board before 9.", 80, 258, 46, D_TEXT, 550, maxWidth=TW - 160, lineHeight=1.15),
         rect(TW - 264, 410, 184, 72, AMBER, 36, end=1.7),
         text("Send", TW - 172, 446, 32, D_TEXT, 700, anchor="center", end=1.7),
         rect(TW - 264, 410, 184, 72, INKF, 36, start=1.7, animate=[{"from": {"scale": 0.94}, "to": {"scale": 1}, "duration": 0.25, "ease": "outExpo"}]),
         text("Sent ✓", TW - 172, 446, 32, "#FBFAF7", 700, anchor="center", start=1.7)]
    BX, BY = 40 + TW - 172, 680 + 446
    S.append(scene(3.0, [ground(1.0), {**vgroup(vclock_layers([(0, M(7, 0)), (3, M(7, 1))], 3.0, color=INKF)), "blur": 12, "opacity": 0.35},
                         vgroup([group(40, 680, L, animate=[mv({"y": 900, "opacity": 0}, {"y": 680, "opacity": 1}, 0.35, "outExpo"),
                                                              mv(None, {"y": -700}, 0.35, "inExpo", 2.6)])]),
                         {"component": "cursor", "x": 0, "y": 0, "anchor": "topLeft", "width": VW, "height": VH,
                          "props": {"path": [[900, 1800], [BX + 6, BY + 4], [BX, BY], [BX, BY]], "times": [0.2, 1.2, 1.45, 3.0],
                                    "clicks": [1.7], "size": 48, "style": "arrow", "color": INKF, "ripple": AMBER, "trail": False}}],
                   cam(mv({"zoom": 1.0}, {"zoom": 1.08}, 1.5), mv(None, {"zoom": 1.0}, 0.6, "inOutCubic", 2.0))))
    items = ["Priya · Thursday 2:30", "Lena · Signed copy today", "Ravi · 10:00 still on", "Mom · Sunday lunch"]
    L = [ground(1.0), claim("Approve.\nSend.", 80, 260, "11vh", D_TEXT)]
    for i, it in enumerate(items):
        at = 0.45 + i * 0.25
        L.append(group(80, 900 + i * 160, [
            rect(0, 0, 920, 128, DAY_CARD, 26, stroke="#14181F10", strokeWidth=2), text(it, 40, 38, 44, D_TEXT, 600),
            rect(720, 28, 168, 72, AMBER, 36, end=at), text("Send", 804, 64, 30, D_TEXT, 700, anchor="center", end=at),
            rect(720, 28, 168, 72, INKF, 36, start=at), text("Sent ✓", 804, 64, 30, "#FBFAF7", 700, anchor="center", start=at)],
            start=i * 0.05, enter={"preset": "fadeUp", "duration": 0.3, "distance": 30, "ease": "outExpo"}))
    S.append(scene(2.0, L))
    # ending: the name made of the morning's sent replies; last 0.5 s returns to night (loop)
    S.append(scene(4.0, [ground(1.0, 0.0, 0.5, "inOutCubic", at=3.5),
                         {"id": "vn", "text": "Relay", "font": DISP, "weight": 800, "size": "15vh", "letterSpacing": "-0.05em",
                          "color": "#FFFFFF", "x": "50%", "y": "42%", "end": 3.5},
                         {**vfill("sent", 200, 3.5, "vn", bg=INKF, ease="inOutSine"), "end": 3.5},
                         text("Reads overnight.\nReady by morning.", 540, 1080, 52, D_TEXT, 600, anchor="center", align="center",
                              start=0.4, end=3.5, enter={"preset": "fadeUp", "duration": 0.4, "distance": 20, "ease": "outExpo"}),
                         vcard(NIGHT[0], None, None, start=3.55,
                               animate=[mv({"y": 920, "opacity": 0}, {"y": 860, "opacity": 1}, 0.35, "outExpo")])],
                   cam(mv({"zoom": 1.1}, {"zoom": 1.0}, 1.0, "outExpo"))))
    return {"size": "9:16", "fps": FPS, "background": INK, "intent": {"pace": "fast"},
            "music": {"style": "pop", "bpm": 120, "key": "D", "mood": "bright",
                      "sections": [{"name": "intro", "bars": 1, "energy": 0.4}, {"name": "build", "bars": 1, "energy": 0.7},
                                   {"name": "drop", "bars": 5, "energy": 1.0, "stop": 2}, {"name": "outro", "bars": 1, "energy": 0.5}],
                      "hits": ["@bar4"], "end": "cut"},
            "sound": "auto", "post": {"grain": 0.02, "vignette": 0.1}, "scenes": S}


if __name__ == "__main__" and len(sys.argv) > 1 and sys.argv[1] == "vertical":
    json.dump(vertical(), open(os.path.join(OUT, "spec-9x16.json"), "w"), indent=1)
    print("spec-9x16.json", sum(s["duration"] for s in vertical()["scenes"]))
