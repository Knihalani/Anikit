# Adapt an `analyze --spec` draft to new content, the way the skill describes:
# keep timing, transitions, camera and tempo; swap in our own copy and subjects.
import json, sys, copy
draft, analysis, out, mode = sys.argv[1:5]
d = json.load(open(draft)); a = json.load(open(analysis))
scenes = d["scenes"]

def chrome(word, dur, env="studio"):
    return {"component": "scene3d", "props": {"env": env, "quality": "draft", "resolution": 0.5,
        "camera": {"keys": [{"t": 0, "pos": [-2.4, 0.9, 7.0], "target": [0, 0.4, 0], "focal": 42, "fstop": 2.4},
                            {"t": dur, "pos": [2.0, 0.8, 6.2], "target": [0, 0.4, 0], "focal": 50, "fstop": 2.2}]},
        "objects": [{"text": word, "height": 0.45, "material": "chrome"},
                    {"type": "sphere", "radius": 0.3, "pos": [1.9, 0.3, 0.9], "material": "glass"}]}}

if mode == "bbb":
    # Structure of the reference: white title cards with blue type, cut against
    # colourful rendered "world" shots, ending on a logo slam and a URL card.
    d["theme"]["colors"].update({"bg": "#FCFCFB", "text": "#1E9BD7", "primary": "#1E9BD7", "accent": "#F2A23A"})
    d["music"] = {"style": "corporate", "bpm": 123, "fit": False, "mood": "playful", "hits": d.get("music", {}).get("hits", [])[:3]}
    cards = iter(["A SMALL TEAM PRESENTS", "ONE BIG BUDGET", "THREE BILLS", "AND ONE GIANT SAVING", "GET READY"])
    worlds = ["caustics", "aurora", "flow-ink", "hero"]
    icons = ["#F2A23A", "#E5484D", "#30A46C", "#8E4EC6"]
    labels = ["rent", "coffee", "streaming", "groceries", "savings", "goals", "plans", "bills"]
    w = 0
    for i, s in enumerate(scenes):
        sh = a["shots"][i]
        t = sh["technique"][0]
        if t == "kinetic-type" or sh["look"] in ("flat-2d", "minimal") and sh["luma"] > 0.7:
            last = i >= len(scenes) - 3
            s["background"] = "#FCFCFB"
            if last and i == len(scenes) - 1:
                s["layers"] = [{"text": "hearth.app", "size": "8vh", "weight": 800, "color": "$primary", "enter": "rise"}]
            elif last or sh["duration"] < 0.2:
                s["layers"] = [{"text": "Hearth", "size": "34vh", "weight": 900, "color": "$primary", "letterSpacing": "-0.04em",
                                "animate": [{"from": {"scale": 1.6, "opacity": 0, "blur": 16}, "duration": 0.3, "ease": "outExpo"}]}]
            else:
                s["layers"] = [{"text": next(cards, "GET READY"), "size": "10vh", "weight": 800, "color": "$primary", "enter": "pop by char from center"}]
        elif t in ("scene3d", "footage-or-scene3d") and w % 4 == 1:
            s["background"] = "radial-gradient(#FFE7C2, #F2A23A)"
            s["layers"] = [chrome("HEARTH", s["duration"])]
            w += 1
        else:
            # A colourful world shot: animated backdrop, one subject, one word.
            s["layers"] = [{"shader": worlds[w % 4]},
                           {"shape": "circle", "width": 300, "height": 300, "fill": icons[w % 4], "y": "48%", "enter": "pop"},
                           {"text": labels[w % len(labels)].upper(), "size": "8vh", "weight": 900, "color": "#FFFFFF", "y": "74%", "enter": "rise"}]
            w += 1
        s["id"] = "s%d-%s" % (i + 1, t)
elif mode == "sintel":
    # Structure of the reference: dark, slow push-ins, crossfades, a fast
    # middle act cut on the beat, a long hold on the end card.
    d["theme"]["colors"].update({"bg": "#050506", "text": "#E9DCCB", "primary": "#C9874A", "accent": "#6E8FA8"})
    d["music"] = {"style": "cinematic-hybrid", "bpm": 123, "key": "Dm", "mood": "dark epic", "fit": False, "hits": d.get("music", {}).get("hits", [])[:4]}
    d["post"] = {"preset": "cinematic", "vignette": 0.45, "grain": 0.02, "bloom": 0.3}
    looks = ["aurora", "light-shafts", "caustics", "flow-ink"]
    words = ["Somewhere north", "the wind turns", "", "we go anyway", "", "", "", "further", "", "", "", "colder", "", "", "", ""]
    k = 0
    for i, s in enumerate(scenes):
        sh = a["shots"][i]
        if i == len(scenes) - 1:
            s["background"] = "#050506"
            s["layers"] = [{"text": "NORTHWIND", "size": "8vh", "weight": 300, "letterSpacing": "0.4em", "color": "$text", "enter": "blur"},
                           {"text": "outerwear for the long way", "size": "2.4vh", "y": "60%", "color": "$primary", "enter": {"preset": "fade", "delay": 1.5}}]
            continue
        if sh["look"] == "minimal":
            s["layers"] = [{"shape": "rect", "width": "100%", "height": "100%", "fill": "#050506"}]
            continue
        lay = [{"shader": looks[k % 4]}]
        if k % 5 == 2:
            lay = [chrome("NORTH", s["duration"], env="night")]
        k += 1
        wd = words[i] if i < len(words) else ""
        if wd:
            lay.append({"text": wd, "size": "4.5vh", "weight": 300, "letterSpacing": "0.2em", "color": "$text", "y": "78%", "enter": "blur"})
        s["layers"] = lay
d.pop("motionBlur", None)
json.dump(d, open(out, "w"), indent=1)
