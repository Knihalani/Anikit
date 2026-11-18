# Benchmark tooling

`tools/bench.py` (Python 3, standard library only):

| Command | What it does |
|---|---|
| `bench.py index` | Parse every `<category>/<slug>.md` front matter → `pieces.json`, and rewrite the tables in `INDEX.md`. Run after adding or editing a piece. |
| `bench.py verify [slug…]` | Re-check each URL through Vimeo/YouTube oEmbed (title and channel), or the page status for studio pages. |
| `bench.py fetch [slug…] [--refresh]` | Download the piece's official stills into the cache, with a `stills.json` manifest that tags each one `frame` (a true frame: YouTube's auto-frames at 25/50/75%, 1280×720 `maxres1–3` when available, else `sd`/`hq`), `studio` (owner/press stills from `stills:`) or `thumb` (a designed YouTube thumbnail or Vimeo poster, which may not be a frame). |
| `bench.py match --spec s.json [--analysis a.json] [--tags "k=v …"] [--text "…"] [-n 3] [--json]` | Profile our video (aspect, pace, medium, type role, palette, purpose) and rank the set; returns the 3 most relevant references from different studios, with reasons and the stills folder. |
| `bench.py export --out DIR [--no-thumbs] [slug…]` | Write the JUDGE layout: `DIR/index.json` (slug, title, studio, year, url, format, category, style_tags, pace, measured, `real_frames`, `frame_info` [{file, kind, thumb, w, h, src}]; true frames first, thumbnails last) and `DIR/<slug>/01.jpg…` copied from the cache (run `fetch` first). From Python: `sys.path.insert(0, "docs/video/judge/benchmark/tools"); from bench import match_refs; match_refs(spec="s.json", analysis=None, tags=None, text=None, n=3)`. |
| `bench.py stats` | Counts and distributions across the set (feeds `WHAT-WORLD-CLASS-LOOKS-LIKE.md`). |
| `clips.json` | Lawfully downloadable reference clips (CC BY / public domain direct files, with licence and fit notes) for motion strips. |
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
