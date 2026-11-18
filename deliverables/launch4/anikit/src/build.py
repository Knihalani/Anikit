#!/usr/bin/env python3
"""Embed src/shells.rhai into a spec template (turns every {"shells": [cues]} into a script layer) so the shipped spec is self-contained."""
import json, sys, pathlib
here = pathlib.Path(__file__).parent
src = (here / "shells.rhai").read_text().splitlines()
def sub(o):
    if o == "@SHELLS@": return src
    if isinstance(o, dict):
        if "shells" in o:  # {"shells": [cues...]} -> script layer that draws them
            return {"script": ["let cues = " + json.dumps(o["shells"]) + ";"] + src}
        return {k: sub(v) for k, v in o.items()}
    if isinstance(o, list): return [sub(v) for v in o]
    return o
spec = sub(json.load(open(sys.argv[1])))
json.dump(spec, open(sys.argv[2], "w"), indent=1)
