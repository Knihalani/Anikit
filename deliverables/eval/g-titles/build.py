#!/usr/bin/env python3
"""Embed tiles.rhai into base.json -> spec.json (the engine's script layer takes inline source only)."""
import json, pathlib
here = pathlib.Path(__file__).parent
spec = json.loads((here / "base.json").read_text())
src = (here / "tiles.rhai").read_text().splitlines()
for layer in spec["scenes"][0]["layers"]:
    if layer.get("script") == "__TILES__":
        layer["script"] = src
(here / "spec.json").write_text(json.dumps(spec, indent=1, ensure_ascii=False))
