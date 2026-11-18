#!/usr/bin/env python3
"""Builds spec.json for the noise-cancelling explainer (30 s, 16:9)."""
import json, pathlib
D = 30.0
def kf(points):  # [(sec, value)] -> [[0..1, value]]
    return [[round(t / D, 4), v] for t, v in points]

uni = {
    "camH":   [(0, 0.32), (4.5, 0.55), (8, 1.9), (11, 2.6), (17, 3.3), (22, 3.6), (25.5, 3.4), (30, 2.3)],
    "camZ":   [(0, 3.4), (4.5, 3.0), (8, -1.2), (11, -1.6), (17, 0.6), (22, 1.2), (25.5, 3.2), (30, 5.75)],
    "pitch":  [(0, 0.10), (4.5, 0.16), (8, 0.3), (11, 0.38), (17, 0.62), (22, 0.74), (25.5, 1.0), (30, 1.47)],
    "camX":   [(0, -1.5), (4.5, -1.1), (8, 0.3), (11, 0.4), (17, 0.0), (30, 0.0)],
    "chaos":  [(0, 0), (5.2, 0), (7.0, 1), (17, 1.15), (20.6, 1.6), (21.0, 1.6), (23.5, 0.0), (30, 0)],
    "ring":   [(0, 0), (11, 0), (12.6, 1), (30, 1)],
    "mic":    [(0, 0), (12.8, 0), (13.4, 1), (14.0, 0.3), (14.6, 1), (15.2, 0.3), (15.8, 1), (17.5, 1), (19, 0.2), (30, 0)],
    "anti":   [(0, 0), (17.2, 0), (18.4, 1), (21.0, 1), (21.4, 0), (30, 0)],
    "cancel": [(0, 0), (19.5, 0), (20.6, 0.55), (21.0, 1), (30, 1)],
    "calm":   [(0, 0), (21.0, 0), (21.6, 0.75), (23.5, 1), (30, 1)],
    "warm":   [(0, 0.1), (5, 0.15), (7.5, 1), (20.6, 1), (21.0, 0.3), (30, 0.0)],
}
anim = [{"keyframes": {k: kf(v) for k, v in uni.items()}, "duration": D, "ease": "inOutSine"}]

W = "Anybody"
def word(text, start, end, y="56%", size="26vh", wd=(150, 60)):
    return {"text": text, "font": W, "size": size, "weight": 800, "color": "#f4efe6", "x": "50%", "y": y,
            "letterSpacing": "-0.03em", "start": start, "end": end, "axes": {"wdth": wd[0]},
            "animate": [{"from": {"opacity": 0}, "duration": 0.6},
                        {"keyframes": {"axes.wdth": [wd[0], wd[1], wd[0]]}, "duration": end - start, "ease": "inOutSine"}],
            "exit": {"preset": "fade", "duration": 0.6}}

channel_layers = [
    word("SOUND", 0, 5.4),
    word("NOISE", 5.0, 11.4, size="30vh", wd=(60, 150)),
    word("LISTEN", 11.0, 17.4, size="20vh"),
    word("FLIP", 17.0, 21.0, size="22vh", wd=(150, 50)),
]

M = "Martian Mono"
def cap(text, start, end, x, y, anchor="left"):
    return {"text": text, "font": M, "size": "2.6vh", "color": "#efe9df", "x": x, "y": y, "anchor": anchor, "align": anchor if anchor in ("left", "right") else "center",
            "letterSpacing": "0.06em", "lineHeight": 1.45, "maxWidth": "30vw", "axes": {"wdth": 100, "wght": 380},
            "start": start, "end": end, "enter": {"preset": "fadeUp", "duration": 0.7}, "exit": {"preset": "fade", "duration": 0.5}}

layers = [
    {"src": "surface.glsl", "uniforms": {}, "channels": [{"layers": channel_layers}, {"layers": [dict(word("SILENCE", 21.2, 30.0, y="80%", size="25vh", wd=(112, 112)), color="#ffffff")]}], "animate": anim},
    cap("Sound is air, pushed into waves:\na pressure ring spreading out.", 1.2, 5.2, "7vw", "84vh"),
    cap("Noise is thousands of them,\narriving at once.", 6.0, 11.0, "7vw", "84vh"),
    cap("A microphone on the earcup\nhears each wave before your ear does.", 12.6, 17.0, "7vw", "84vh"),
    cap("The speaker plays it back flipped.\nPeak meets trough. They cancel.", 17.6, 20.8, "7vw", "84vh"),
    dict(cap("Active noise cancellation:\nsilence, made from sound.", 23.5, 30.0, "7vw", "84vh"), color="#1b1a18"),
]
spec = {
    "size": "16:9", "fps": 30, "background": "#020203",
    "intent": {"pace": "medium"},
    "encode": {"crf": 24},
    "sound": {"design": "off", "loudness": -14, "space": "dry"},
    "audio": [{"src": "soundtrack.wav", "at": 0}],
    "scenes": [{"duration": D, "layers": layers}],
}
p = pathlib.Path(__file__).with_name("spec.json")
p.write_text(json.dumps(spec, indent=1))
print(p)
