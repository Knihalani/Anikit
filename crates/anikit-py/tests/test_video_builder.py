"""The Python spec builder produces the same spec as the TypeScript one."""

import json
import os

import pytest

from anikit._video_builder import AnikitVideoError, absolutize, video

FIXTURE = os.path.join(os.path.dirname(__file__), "..", "..", "anikit-video", "tests", "fixtures", "builder.json")


def build():
    """The same video as packages/anikit/test/video-builder.test.ts builds."""
    v = (video(size="9:16", fps=30)
         .theme(colors={"primary": "#7c5cff"})
         .transition({"type": "crossfade", "duration": 0.4})
         .marker("drop", 2)
         .scene(lambda s: s
                .text("Ship faster", style="title", split="words", enter={"preset": "fadeUp", "stagger": 0.08})
                .shape("circle", width=200, height=200, fill="$primary", loop="float", animate="pop"), duration=2.5))
    with v.scene(duration=3, transition={"type": "push", "direction": "left"}) as s:
        s.camera(animate=[{"to": {"zoom": 1.05}, "duration": 3, "ease": "inOutSine"}])
        with s.box(direction="row", gap=24, stagger=0.1) as row:
            row.text("Fast", style="heading", animate="rise").text("Native", style="heading", enter="fadeUp")
        s.image("shot.png", fit="cover", width="80%", y="70%")
        s.component("barChart", {"values": [3, 5, 8]}, enter="fade")
    return v.audio("music.mp3", volume=0.8, fade_out=1)


def test_matches_shared_fixture():
    with open(FIXTURE, encoding="utf-8") as f:
        assert build().to_json() == json.load(f)


def test_snake_case_and_aliases():
    spec = video().scene(lambda s: s.text("x", letter_spacing="0.1em", animate="type")).to_json()
    assert spec["scenes"][0]["layers"][0] == {"text": "x", "letterSpacing": "0.1em", "enter": "typewriter"}


def test_absolutize():
    spec = absolutize(build().to_json(), "/proj")
    assert spec["scenes"][1]["layers"][1]["src"] == "/proj/shot.png"
    assert spec["audio"][0]["src"] == "/proj/music.mp3"


def test_missing_binary_is_actionable():
    with pytest.raises(AnikitVideoError, match="ANIKIT_VIDEO_BIN"):
        build().render("out.mp4", bin="/nonexistent/anikit-video")


def test_component_kinds_and_suggestions():
    from anikit._video_builder import COMPONENT_KINDS
    assert "bar-chart" in COMPONENT_KINDS and "mesh-gradient" in COMPONENT_KINDS
    with pytest.warns(UserWarning, match="did you mean 'bar-chart'"):
        video().scene(lambda s: s.component("bar-chrt", {"data": [1, 2]}))
