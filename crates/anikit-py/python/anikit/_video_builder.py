"""Anikit Video spec builder: the Python twin of the TypeScript ``anikit/video``.

Build a JSON spec fluently, then render it with the native ``anikit-video``
CLI (no browser)::

    from anikit.video import video

    (video(size="1080p")
        .scene(lambda s: s.text("Hello", style="title", animate="rise"), duration=3)
        .render("out.mp4"))

    # or with context managers
    v = video(size="9:16")
    with v.scene(duration=3) as s:
        s.text("Ship faster", style="title", split="words", enter={"preset": "fadeUp", "stagger": 0.08})
        with s.box(direction="row", gap=24) as row:
            row.text("Fast").text("Native")
    v.render("teaser.mp4")

Keyword arguments may be snake_case (``letter_spacing``); they are written as
the spec's camelCase keys (``letterSpacing``). ``animate="rise"`` is a
shorthand for an entrance preset. The spec reference is
``docs/video/spec.md``; set ``ANIKIT_VIDEO_BIN`` to point at the binary.
"""

from __future__ import annotations

import copy
import difflib
import warnings
import json
import os
import subprocess
import tempfile
from typing import Any, Callable, Dict, List, Optional, Union

from ._video_components import COMPONENT_KINDS

__all__ = [
    "COMPONENT_KINDS",
    "video", "Video", "SceneBuilder", "LayerList", "template", "render", "absolutize", "PRESET_ALIASES",
    "AnikitVideoError",
]

#: Friendly aliases accepted wherever a preset name is (``animate="rise"``).
PRESET_ALIASES = {
    "rise": "fadeUp", "fade": "fadeIn", "pop": "popIn", "drop": "fadeDown", "type": "typewriter",
    "draw": "drawOn", "zoom": "zoomIn", "blur": "blurIn", "slide": "slideUp", "count": "countUp",
}

_BOX_STYLE_KEYS = {"direction", "gap", "padding", "align", "justify", "wrap", "grow", "background", "radius", "border", "columns"}

Json = Dict[str, Any]


class AnikitVideoError(RuntimeError):
    """The ``anikit-video`` CLI failed or is missing; the message says how to fix it."""


def _camel(key: str) -> str:
    head, *rest = key.split("_")
    return head + "".join(p[:1].upper() + p[1:] for p in rest)


def _opts(kw: Json) -> Json:
    out = {_camel(k): v for k, v in kw.items() if v is not None}
    anim = out.get("animate")
    if isinstance(anim, str):
        out.setdefault("enter", PRESET_ALIASES.get(anim, anim))
        del out["animate"]
    for k in ("enter", "exit"):
        if isinstance(out.get(k), str):
            out[k] = PRESET_ALIASES.get(out[k], out[k])
    return out


class LayerList:
    """Adds layers; shared by scenes, boxes and groups. Every method returns ``self``."""

    def __init__(self) -> None:
        self.layers: List[Any] = []

    def __enter__(self) -> "LayerList":
        return self

    def __exit__(self, *exc: Any) -> None:
        return None

    def text(self, text: str, **opts: Any) -> "LayerList":
        """Add a text layer (``style``, ``size``, ``color``, ``split``, ``enter``, …)."""
        self.layers.append({"text": text, **_opts(opts)})
        return self

    def shape(self, shape: str, **opts: Any) -> "LayerList":
        """Add a shape: ``rect``, ``circle``, ``ellipse``, ``line``, ``arrow``, ``polygon``, ``star``, ``path``."""
        self.layers.append({"type": "shape", "shape": shape, **_opts(opts)})
        return self

    def image(self, src: str, **opts: Any) -> "LayerList":
        """Add an image (PNG, JPEG, WebP, GIF, SVG)."""
        self.layers.append({"type": "image", "src": src, **_opts(opts)})
        return self

    def video(self, src: str, **opts: Any) -> "LayerList":
        """Add a video clip (MP4, MOV, WebM)."""
        self.layers.append({"type": "video", "src": src, **_opts(opts)})
        return self

    def box(self, build: Optional[Callable[["LayerList"], Any]] = None, **opts: Any) -> "LayerList":
        """Add a flexbox container.

        ``box(lambda b: b.text("A"), style={...})`` returns ``self``; without
        ``build`` it returns the child list, for ``with s.box(direction="row") as row:``.
        Bare style keys (``direction``, ``gap``, …) are moved into ``style``.
        """
        style = {k: opts.pop(k) for k in list(opts) if k in _BOX_STYLE_KEYS}
        o = _opts(opts)
        if style:
            o["style"] = {**o.get("style", {}), **{_camel(k): v for k, v in style.items()}}
        kids = LayerList()
        self.layers.append(_Lazy({"type": "box", **o}, "children", kids))
        if build is None:
            return kids
        build(kids)
        return self

    def group(self, build: Optional[Callable[["LayerList"], Any]] = None, **opts: Any) -> "LayerList":
        """Add a transformable group; like :meth:`box`, returns the child list without ``build``."""
        kids = LayerList()
        self.layers.append(_Lazy({"type": "group", **_opts(opts)}, "layers", kids))
        if build is None:
            return kids
        build(kids)
        return self

    def component(self, component: str, props: Optional[Json] = None, **opts: Any) -> "LayerList":
        """Add a component (``barChart``, ``code``, ``browser``, …)."""
        kebab = "".join("-" + c.lower() if c.isupper() else c for c in component).replace("_", "-")
        if "." not in component and kebab not in COMPONENT_KINDS:
            near = difflib.get_close_matches(kebab, list(COMPONENT_KINDS), n=1)
            warnings.warn(f"anikit.video: unknown component kind '{component}'" + (f", did you mean '{near[0]}'?" if near else ""), stacklevel=2)
        layer: Json = {"type": "component", "component": component}
        if props:
            layer["props"] = props
        layer.update(_opts(opts))
        self.layers.append(layer)
        return self

    def layer(self, layer: Json) -> "LayerList":
        """Add a raw layer dict (escape hatch)."""
        self.layers.append(layer)
        return self

    def _json(self) -> List[Any]:
        return [x.to_json() if isinstance(x, _Lazy) else x for x in self.layers]


class _Lazy:
    """A container whose children are serialised when the spec is built."""

    def __init__(self, head: Json, key: str, kids: LayerList) -> None:
        self.head, self.key, self.kids = head, key, kids

    def to_json(self) -> Json:
        return {**self.head, self.key: self.kids._json()}


class SceneBuilder(LayerList):
    """Builds one scene; also a context manager (``with v.scene(duration=3) as s:``)."""

    def __init__(self, **opts: Any) -> None:
        super().__init__()
        self.options: Json = _opts(opts)

    def duration(self, d: Union[float, str]) -> "SceneBuilder":
        """Scene length in seconds (or ``"auto"``)."""
        self.options["duration"] = d
        return self

    def transition(self, t: Union[str, Json]) -> "SceneBuilder":
        """How this scene comes in from the previous one."""
        self.options["transition"] = t
        return self

    def background(self, c: str) -> "SceneBuilder":
        """Scene background colour or gradient."""
        self.options["background"] = c
        return self

    def camera(self, c: Optional[Json] = None, **kw: Any) -> "SceneBuilder":
        """Scene camera (``zoom``, ``x``, ``y``, ``rotation``, ``animate``, ``shake``)."""
        self.options["camera"] = {**(c or {}), **_opts(kw)}
        return self

    def marker(self, name: str, at: Union[float, str]) -> "SceneBuilder":
        """A named time inside this scene, usable as ``"@name"``."""
        self.options.setdefault("markers", {})[name] = at
        return self

    def audio(self, src: str, **opts: Any) -> "SceneBuilder":
        """An audio cue relative to the scene start."""
        self.options.setdefault("audio", []).append({"src": src, **_opts(opts)})
        return self

    def to_json(self) -> Json:
        """The scene as spec JSON."""
        return {**self.options, "layers": self._json()}


class Video:
    """A video under construction. Build with :func:`video`."""

    def __init__(self, **opts: Any) -> None:
        self._spec: Json = _opts(opts)
        self._scenes: List[SceneBuilder] = []

    def scene(self, build: Optional[Callable[[SceneBuilder], Any]] = None, **opts: Any) -> Any:
        """Add a scene. ``scene(lambda s: s.text("Hi"), duration=3)`` returns the video;
        ``scene(duration=3)`` returns the :class:`SceneBuilder` (use it with ``with``)."""
        s = SceneBuilder(**opts)
        self._scenes.append(s)
        if build is None:
            return s
        build(s)
        return self

    def transition(self, t: Union[str, Json]) -> "Video":
        """Default transition between scenes."""
        self._spec["transition"] = t
        return self

    def theme(self, theme: Optional[Json] = None, **kw: Any) -> "Video":
        """Theme colours, fonts and styles (merged with what is set)."""
        t = {**(theme or {}), **kw}
        cur = self._spec.setdefault("theme", {})
        for k, v in t.items():
            if isinstance(v, dict) and isinstance(cur.get(k), dict):
                cur[k] = {**cur[k], **v}
            else:
                cur[k] = v
        return self

    def marker(self, name: str, at: Union[float, str]) -> "Video":
        """A named time on the global timeline (``"@name"``)."""
        self._spec.setdefault("markers", {})[name] = at
        return self

    def audio(self, src: str, **opts: Any) -> "Video":
        """Music or a sound effect (``volume``, ``fade_in``, ``fade_out``, ``at``, ``duck``, …)."""
        self._spec.setdefault("audio", []).append({"src": src, **_opts(opts)})
        return self

    def captions(self, captions: Any) -> "Video":
        """Burned-in captions: a list of cues, or ``{"src": "subs.srt"}``."""
        self._spec["captions"] = captions
        return self

    def font(self, family: str, path: str) -> "Video":
        """Register a font file under a family name."""
        self._spec.setdefault("fonts", {})[family] = path
        return self

    def to_json(self) -> Json:
        """The spec as a dict."""
        spec = copy.deepcopy(self._spec)
        spec["scenes"] = [s.to_json() for s in self._scenes]
        # Same key order as the TypeScript builder: scenes after the top-level options.
        order = ["size", "fps", "background", "theme", "transition", "markers", "scenes", "audio", "captions", "fonts", "duration"]
        return {k: spec[k] for k in sorted(spec, key=lambda k: order.index(k) if k in order else len(order))}

    spec = to_json

    def save(self, path: str) -> str:
        """Write the spec to a JSON file."""
        with open(path, "w", encoding="utf-8") as f:
            json.dump(self.to_json(), f, indent=2)
            f.write("\n")
        return path

    def render(self, out: str, quality: Optional[str] = None, **run: Any) -> str:
        """Render to a video file (.mp4, .webm, .mov, .gif). Returns the output path."""
        _run_spec(self.to_json(), ["render", "{spec}", "-o", out] + (["--quality", quality] if quality else []), **run)
        return out

    def still(self, at: float, out: str, **run: Any) -> str:
        """Render one frame at ``at`` seconds to a PNG."""
        _run_spec(self.to_json(), ["still", "{spec}", "--at", str(at), "-o", out], **run)
        return out

    def sheet(self, out: str, **run: Any) -> str:
        """Render a contact sheet (one frame per scene) to a PNG."""
        _run_spec(self.to_json(), ["sheet", "{spec}", "-o", out], **run)
        return out

    def check(self, **run: Any) -> Json:
        """Validate with the renderer: ``{"ok", "errors", "warnings", "duration", …}``."""
        return json.loads(_run_spec(self.to_json(), ["check", "{spec}", "--json"], allow_fail=True, **run))

    def critique(self, **run: Any) -> Json:
        """Heuristic design review: reading time, contrast, safe areas, pacing."""
        return json.loads(_run_spec(self.to_json(), ["critique", "{spec}", "--json"], **run))


def video(**opts: Any) -> Video:
    """Start a video: ``video(size="9:16", fps=30, theme={...}, transition=...)``."""
    return Video(**opts)


def absolutize(spec: Any, base_dir: str) -> Any:
    """Make relative ``src``/``logo``/font paths absolute, so a temporary spec file finds them."""

    def rel(s: str) -> bool:
        return bool(s) and not os.path.isabs(s) and "://" not in s and not s.startswith(("data:", "$"))

    if isinstance(spec, list):
        return [absolutize(x, base_dir) for x in spec]
    if isinstance(spec, dict):
        out = {}
        for k, v in spec.items():
            if k in ("src", "logo") and isinstance(v, str) and rel(v):
                out[k] = os.path.join(base_dir, v)
            elif k == "fonts" and isinstance(v, dict):
                out[k] = {f: (os.path.join(base_dir, p) if isinstance(p, str) and rel(p) and ("." in p or "/" in p) else p) for f, p in v.items()}
            else:
                out[k] = absolutize(v, base_dir)
        return out
    return spec


def _exec(args: List[str], bin: Optional[str] = None, extra: Optional[List[str]] = None, allow_fail: bool = False) -> str:
    exe = bin or os.environ.get("ANIKIT_VIDEO_BIN") or "anikit-video"
    try:
        p = subprocess.run([exe, *args, *(extra or [])], capture_output=True, text=True)
    except FileNotFoundError:
        raise AnikitVideoError(
            f"anikit-video: binary '{exe}' not found. Build it with `cargo build --release -p anikit-video` "
            "and set ANIKIT_VIDEO_BIN, or put it on PATH."
        ) from None
    if p.returncode != 0 and not (allow_fail and p.stdout):
        raise AnikitVideoError(f"anikit-video {args[0]} failed:\n{p.stderr or p.stdout}")
    return p.stdout


def _run_spec(spec: Json, args: List[str], base_dir: Optional[str] = None, bin: Optional[str] = None,
              extra: Optional[List[str]] = None, allow_fail: bool = False) -> str:
    fixed = absolutize(spec, os.path.abspath(base_dir or "."))
    fd, path = tempfile.mkstemp(prefix="anikit-video-", suffix=".json")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(fixed, f)
        return _exec([path if a == "{spec}" else a for a in args], bin=bin, extra=extra, allow_fail=allow_fail)
    finally:
        os.unlink(path)


def template(name: str, params: Json, brand: Union[None, str, Json] = None, format: str = "16:9",
             bin: Optional[str] = None) -> Json:
    """Expand a built-in template (launch, feature, changelog, demo, teaser, data-story,
    testimonial, tutorial, countdown, logo-reveal) into a spec dict."""
    tmp: List[str] = []
    try:
        def write(obj: Any) -> str:
            fd, p = tempfile.mkstemp(prefix="anikit-video-", suffix=".json")
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                json.dump(obj, f)
            tmp.append(p)
            return p

        args = ["templates", "expand", name, write(params), "--format", format]
        if brand is not None:
            args += ["--brand", brand if isinstance(brand, str) else write(brand)]
        return json.loads(_exec(args, bin=bin))
    finally:
        for p in tmp:
            os.unlink(p)


def render(spec: Json, out: str, **run: Any) -> str:
    """Render a spec dict (from :func:`template` or written by hand)."""
    _run_spec(spec, ["render", "{spec}", "-o", out], **run)
    return out
