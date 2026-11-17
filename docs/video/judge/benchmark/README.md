# Benchmark tooling

`tools/bench.py` (Python 3, standard library only):

| Command | What it does |
|---|---|
| `bench.py index` | Parse every `<category>/<slug>.md` front matter → `pieces.json`, and rewrite the tables in `INDEX.md`. Run after adding or editing a piece. |
| `bench.py verify [slug…]` | Re-check each URL through Vimeo/YouTube oEmbed (title and channel), or the page status for studio pages. |
| `bench.py fetch [slug…]` | Download the piece's official stills into the cache: YouTube's own frames (`maxresdefault`, `hq1–3` at 25/50/75%), the Vimeo poster frame, and the studio/press stills listed in `stills:`. |
| `bench.py match --spec s.json [--analysis a.json] [--tags "k=v …"] [--text "…"] [-n 3] [--json]` | Profile our video (aspect, pace, medium, type role, palette, purpose) and rank the set; returns the 3 most relevant references from different studios, with reasons and the stills folder. |
| `bench.py stats` | Counts and distributions across the set (feeds `WHAT-WORLD-CLASS-LOOKS-LIKE.md`). |
| `bench.py measure <slug> <clip>` | Run `anikit-video analyze` on a legitimately obtained clip, store the key stats in `measured.json` and a contact sheet in the cache. |

## Cache (never committed)

Stills and clips go to `$ANIKIT_BENCH_CACHE`, default `<repo>/.cache/benchmark/<slug>/` (listed in `.gitignore`). They are for evaluation only. Do not commit them, attach them to issues or redistribute them.

A clip is only analysed when we may legitimately hold a copy: a Creative Commons licence (e.g. Blender Studio's open movies, CC BY), an official press kit download, or a download the owner enabled. We do not rip YouTube or Vimeo streams.

## Examples

```sh
# a premium 3D product launch, 16:9, fast
python3 docs/video/judge/benchmark/tools/bench.py match --tags "medium=3d-cg purpose=launch pace=fast mood=premium"
# from one of our specs plus the analysis of its render
target/release/anikit-video analyze out.mp4 -o out.analysis.json
python3 docs/video/judge/benchmark/tools/bench.py match --spec film.json --analysis out.analysis.json
```
