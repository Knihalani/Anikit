"""Python builder: python examples/video/build.py (needs the anikit package and the anikit-video binary;
set ANIKIT_VIDEO_BIN=target/release/anikit-video)."""

import json
import os

from anikit.video import template, render, video

here = os.path.dirname(os.path.abspath(__file__))

# 1. Hand-built, with context managers.
v = video(size="9:16", fps=30).theme(colors={"primary": "#7C5CFF", "accent": "#FF5CA8"})
with v.scene(duration=2.6) as s:
    s.text("Ship the video", style="title", split="words", enter={"preset": "fadeUp", "stagger": 0.08, "ease": "outExpo"})
    s.text("before the meeting.", style="subtitle", y="60%", animate="rise")
with v.scene(duration=3, transition={"type": "push", "direction": "up"}) as s:
    s.text("{value:,} fps", value=935, style="title", enter="countUp")
    s.text("on a laptop", style="subtitle", y="62%", enter={"preset": "fadeUp", "delay": 0.6})
v.render("teaser.mp4")

# 2. From a template plus a brand kit, in all three formats.
with open(os.path.join(here, "launch", "params.json")) as f:
    params = json.load(f)
for fmt in ("16:9", "9:16", "1:1"):
    spec = template("launch", params, brand=os.path.join(here, "brand.json"), format=fmt)
    render(spec, f"launch-{fmt.replace(':', 'x')}.mp4")
