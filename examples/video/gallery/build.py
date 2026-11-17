"""Component gallery: every component kind in two clearly different looks, 1080p60.

    python examples/video/gallery/build.py            # writes components.json next to this file
    anikit-video render examples/video/gallery/components.json -o gallery.mp4

The kind list comes from `anikit-video info --json`, so new kinds appear automatically
(with their built-in example props); OVERRIDES below adds richer props per kind and look.
Set ANIKIT_VIDEO_BIN to the binary (default target/release/anikit-video).
"""

import json
import os
import subprocess
import sys

here = os.path.dirname(os.path.abspath(__file__))
root = os.path.abspath(os.path.join(here, "..", "..", ".."))
binary = os.environ.get("ANIKIT_VIDEO_BIN", os.path.join(root, "target", "release", "anikit-video"))
SECONDS = float(os.environ.get("GALLERY_SECONDS", "1.5"))  # per look; two looks per kind

# Two deliberately different art directions. Each is a component theme plus a scene background.
LOOKS = {
    "studio": {  # graphite, neutral, warm amber accent
        "background": "#0c0d10",
        "theme": {"mode": "dark", "background": "#0c0d10", "surface": "#16181d", "accent": "#f5a524",
                  "palette": ["#f5a524", "#e8e6e1", "#7b8794", "#4cc38a", "#e5484d"], "radius": 14},
        "caption": "#8b8f98",
    },
    "paper": {  # editorial light: cream paper, ink, vermilion
        "background": "#f3efe6",
        "theme": {"mode": "light", "background": "#f3efe6", "surface": "#fffdf8", "text": "#16140f",
                  "accent": "#e2462b", "palette": ["#e2462b", "#16140f", "#2f6f6a", "#d9a441", "#7d6b5d"],
                  "radius": 4},
        "caption": "#6d675c",
    },
}

# Per-kind props per look (merged over the kind's built-in example). Keys are kinds; values
# are {look: props} or a single props dict used for both looks.
OVERRIDES = {
    "code": {"studio": {"minimap": True, "statusBar": True, "code": "export async function render(spec) {\n  const frames = await compile(spec)\n  for (const f of frames) encode(f)\n  return frames.length // 60 fps\n}", "language": "ts"},
             "paper": {"surface": "paper", "chrome": "minimal", "radius": 6, "code": "def render(spec):\n    frames = compile(spec)\n    for f in frames:\n        encode(f)\n    return len(frames)", "language": "python"}},
    "terminal": {"lines": ["$ anikit-video render launch.json", "compiled 4 scenes in 12 ms", "rendered 600 frames at 214 fps", "wrote launch.mp4 (4.1 MB)"]},
    "bar-chart": {"studio": {"values": [42, 68, 55, 91, 77], "labels": ["Mon", "Tue", "Wed", "Thu", "Fri"]},
                  "paper": {"values": [42, 68, 55, 91, 77], "labels": ["Mon", "Tue", "Wed", "Thu", "Fri"], "shape": "square", "fill": "solid", "dashed": True, "motion": "snappy"}},
    "line-chart": {"series": [[12, 18, 15, 26, 24, 33, 41, 38, 52]]},
    "area-chart": {"series": [[8, 14, 11, 20, 26, 23, 31, 40], [4, 6, 9, 8, 12, 15, 14, 19]]},
    "donut-chart": {"values": [48, 27, 15, 10], "labels": ["Direct", "Search", "Social", "Email"]},
    "pie-chart": {"values": [48, 27, 15, 10]},
    "counter": {"to": 1284500, "prefix": "$"},
    "stat": {"label": "Monthly revenue", "value": 48200, "prefix": "$", "delta": 12.4, "spark": [3, 4, 3, 5, 6, 8, 7, 9]},
    "phone": {"studio": {"screen": "chat", "title": "Support", "finish": "blue", "ui": "dark", "tint": "accent"},
              "paper": {"screen": "feed", "title": "Journal", "finish": "desert", "ui": "light", "tint": "warm", "font": "display", "radius": 20, "density": "large", "motion": "lively"}},
    "laptop": {"studio": {"screen": "dashboard", "title": "Northwind", "motion": "lively"},
               "paper": {"screen": "editor", "title": "Drafts", "finish": "starlight", "ui": "light", "tint": "warm", "font": "serif", "radius": 4, "density": "compact"}},
    "browser": {"studio": {"url": "northwind.app", "screen": "app", "title": "Ship faster", "style": "chrome", "ui": "dark", "tint": "cool", "font": "mono", "radius": 2},
                "paper": {"url": "journal.press", "screen": "settings", "title": "Settings", "style": "safari", "ui": "light"}},
    "card": {"title": "Realtime sync", "body": "Every edit lands everywhere in under 50 ms."},
    "lower-third": {"studio": {"title": "Ada Lovelace", "subtitle": "Head of Research, Analytical Engines", "style": "plate"},
                    "paper": {"title": "Ada Lovelace", "subtitle": "Head of Research, Analytical Engines", "style": "minimal"}},
    "title-card": {"studio": {"eyebrow": "Introducing", "title": "Video, rendered by agents", "subtitle": "Native speed. Studio quality.", "chip": True, "sheen": True},
                   "paper": {"eyebrow": "Chapter one", "title": "The quiet machine", "subtitle": "An essay in motion", "face": "serif", "reveal": "fade", "align": 0}},
    "gradient": {"studio": {"colors": ["#0c0d10", "#2a1d0a"]}, "paper": {"colors": ["#f3efe6", "#e9dcc6"]}},
    "particle-text": {"studio": {"text": "SHIP"}, "paper": {"text": "ink"}},
    "text-3d": {"studio": {"text": "ANIKIT"}, "paper": {"text": "PRESS"}},
    "globe": {"arcs": [[37.8, -122.4, 51.5, -0.1], [51.5, -0.1, 35.7, 139.7], [35.7, 139.7, -33.9, 151.2], [40.7, -74.0, -23.5, -46.6]]},
    "chat": {"messages": [{"from": "user", "text": "Make a launch video for our API"},
                          {"from": "assistant", "text": "Done: 24 s, 1080p60, with a voiceover and captions."}]},
    "checklist": {"studio": {"items": ["Compile the spec", "Render 1080p60", "Mix the soundtrack", "Publish"]},
                  "paper": {"items": ["Compile the spec", "Render 1080p60", "Mix the soundtrack", "Publish"], "shape": "circle", "surface": "paper", "strike": True}},
    "quote": {"studio": {"text": "We replaced a week of motion design with one prompt.", "name": "Grace Hopper", "title": "VP Product"},
              "paper": {"text": "We replaced a week of motion design with one prompt.", "name": "Grace Hopper", "title": "VP Product", "face": "serif", "align": 0.5}},
    "table": {"columns": ["", "Anikit", "Others"], "rows": [["Native renderer", True, False], ["Agent-first spec", True, False], ["1080p60", True, True]], "highlight": 1},
    "timeline": {"items": [{"label": "Jan", "title": "Private beta"}, {"label": "Mar", "title": "Public launch"}, {"label": "Jun", "title": "1.0"}], "current": 1},
    "marquee": {"items": ["Northwind", "Contoso", "Globex", "Initech", "Umbrella", "Hooli"]},
    "lottie": None,  # needs a file; skipped unless examples/video/assets has one
}


def kinds():
    out = subprocess.run([binary, "info", "--json"], check=True, capture_output=True, text=True).stdout
    return json.loads(out)["components"]


def props_for(kind, example, look):
    o = OVERRIDES.get(kind, {})
    if o and look in o:
        o = o[look]
    elif o and set(o) & set(LOOKS):
        o = {}
    p = dict(example)
    p.update(o or {})
    p["theme"] = LOOKS[look]["theme"]
    return p


FULL_BLEED = {"mesh-gradient", "aurora", "grain", "grid", "dots", "light-leak", "bokeh", "starfield", "gradient",
              "god-rays", "lens-flare", "confetti", "sparks", "snow", "dust", "wipe", "iris", "glitch", "spotlight",
              "ken-burns", "tile-reveal", "kinetic-grid", "reactions", "visualizer", "cursor", "callout", "arrow"}
OVERLAY_KINDS = {"grain", "wipe", "iris", "glitch", "spotlight", "lens-flare", "confetti", "sparks", "snow", "dust", "reactions"}


def build():
    scenes = []
    for k in kinds():
        kind = k["name"]
        if kind in OVERRIDES and OVERRIDES[kind] is None:
            continue
        for look, L in LOOKS.items():
            layers = []
            if kind in OVERLAY_KINDS - {"lens-flare"}:
                # give overlays something to sit on
                layers.append({"type": "component", "component": "mesh-gradient", "props": {"theme": L["theme"]}})
            layer = {"type": "component", "component": kind, "props": props_for(kind, k["example"], look)}
            if kind not in FULL_BLEED:
                layer.update({"width": 1680, "height": 900, "y": 580})  # leave room for the caption
            layers.append(layer)
            pill = "#000000b3" if LOOKS[look]["theme"]["mode"] == "dark" else "#ffffffd9"
            layers.append({"children": [{"type": "text", "text": f"{kind}  ·  {look}", "style": "caption", "color": L["caption"], "size": 20}],
                           "style": {"background": pill, "padding": [6, 14], "radius": 999},
                           "anchor": "topLeft", "x": 24, "y": 24})
            scenes.append({"id": f"{kind}-{look}", "duration": SECONDS, "background": L["background"],
                           "transition": "cut", "layers": layers})
    return {"size": "16:9", "fps": 60, "scenes": scenes}


if __name__ == "__main__":
    spec = build()
    path = os.path.join(here, "components.json")
    with open(path, "w") as f:
        json.dump(spec, f, indent=1)
        f.write("\n")
    print(f"{path}: {len(spec['scenes'])} scenes, {sum(s['duration'] for s in spec['scenes']):.0f} s", file=sys.stderr)
